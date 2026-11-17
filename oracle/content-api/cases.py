"""The core/api corpus: every ContentAPI entry point (each overload and default form is its own `fn` key) and the other
core/api files, with the branches, answers and injected failures that pin each body. A case is one call plus optional
follow-up steps; `sweep` cases are repeated with every recorded call failing in turn."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
GOD = (ROOT / 'reference/server/Server/src/main/core/api/God.kt').read_text(encoding='utf-8')


def god_items():
    out = {}
    for name, body in re.findall(r'(ARMADYL|BANDOS|SARADOMIN|ZAMORAK|GUTHIX)\((.*?)\)', GOD, re.S):
        out[name] = [int(i) for i in re.findall(r'Items\.\w+?_(\d+)\b', body)]
    return out


def loc(x, y, z=0):
    return {'loc': [x, y, z]}


def i(n):
    return {'int': n}


def node(k):
    return {'node': k}


PLAYER = {'p': {'kind': 'player', 'location': loc(3200, 3200), 'username': 'Bob'}}
ITEMS = {
    '995': {'name': 'Coins', 'stackable': True},
    '1511': {'name': 'Logs', 'noteId': 1512, 'config': {'equipment_slot': 3}},
    '1512': {'name': 'Logs', 'unnoted': False, 'noteId': 1511, 'stackable': True},
    '2572': {'name': 'Ring of wealth', 'config': {'equipment_slot': 12}},
    '1540': {'name': 'Anti-dragon shield', 'config': {'equipment_slot': 5, 'rare_item': True}},
    '229': {'name': 'Vial'},
    '2428': {'name': 'Attack potion(4)'}, '121': {'name': 'Attack potion(3)'}, '123': {'name': 'Attack potion(2)'}, '125': {'name': 'Attack potion(1)'},
    '113': {'name': 'Strength potion(4)'}, '115': {'name': 'Strength potion(3)'}, '117': {'name': 'Strength potion(2)'}, '119': {'name': 'Strength potion(1)'},
    '9083': {'name': 'Seal of passage', 'config': {'equipment_slot': 2}},
    '7681': {'name': 'Game book'},
    '1': {'name': 'Toolkit'},
    '65535': {'name': 'null'},
}
for ids in god_items().values():
    for gid in ids:
        ITEMS.setdefault(str(gid), {'name': f'God item {gid}', 'config': {'equipment_slot': 4}})

FUNCTIONS = []
_cases = []


def case(fn, args=(), name=None, nodes=None, answers=None, world=None, steps=None, sweep=False, varbits=None, seed=None, items=True):
    if fn not in FUNCTIONS:
        FUNCTIONS.append(fn)
    c = dict(name=name or f'{fn}#{sum(1 for x, _ in _cases if x["fn"] == fn) + 1}', fn=fn, args=list(args))
    n = dict(PLAYER)
    n.update(nodes or {})
    c['nodes'] = n
    defs = {}
    if items:
        defs['items'] = ITEMS
    if varbits:
        defs['varbits'] = varbits
    c['definitions'] = defs
    if answers:
        c['answers'] = answers
    if world:
        c['world'] = world
    if steps:
        c['steps'] = steps
    if seed is not None:
        c['seed'] = seed
    _cases.append((c, sweep))


def item(id, amount=1, charge=1000, slot=-1, **kw):
    d = dict(kind='item', id=id, amount=amount, charge=charge, slot=slot)
    d.update(kw)
    return d


NPC = {'n': {'kind': 'npc', 'id': 1, 'location': loc(3201, 3201)}}
P2 = {'p2': {'kind': 'player', 'location': loc(3205, 3205), 'username': 'Al', 'male': False}}


def skills():
    case('getTool', ['p', True], answers={'SkillingTool.getPickaxe': ['RUNE_PICKAXE']})
    case('getTool', ['p', False], answers={'SkillingTool.getHatchet': [None]})
    for level, need in ((10, 10), (9, 10)):
        case('hasLevelDyn', ['p', 8, need], answers={'Skills.getLevel': [level]}, sweep=level == 10)
        case('hasLevelStat', ['p', 8, need], answers={'Skills.getStaticLevel': [level]})
    case('rewardXP', ['p', 5, 12.5], sweep=True)
    case('setTempLevel', ['n', 2, 7], nodes=NPC)
    case('getStatLevel', ['p', 3], answers={'Skills.getStaticLevel': [42]})
    case('getDynLevel', ['n', 3], nodes=NPC, answers={'Skills.getLevel': [17]})
    case('adjustLevel', ['p', 2, 5], answers={'Skills.getStaticLevel': [50]}, sweep=True)
    case('adjustLevel', ['p', 2, -60], answers={'Skills.getStaticLevel': [50]})
    case('heal', ['p', 4], answers={'Skills.heal': [3]})
    case('getMasteredSkillNames', ['p'], answers={'Skills.getMasteredSkills': [0]})
    case('getMasteredSkillNames', ['p'], answers={'Skills.getMasteredSkills': [2], 'Skills.getStaticLevel': [99, 1, 99] + [1] * 20 + [99]}, sweep=True)
    case('getFamiliarBoost', ['p', 9], answers={'FamiliarManager.getBoost': [3]})
    for amount in (2.5, -3.0, 0.0):
        case('modPrayerPoints', ['p', amount])
    for shield in (None, 2890, 9731, 1540, 11283, 11284, 1511):
        nodes = {'s': item(shield)} if shield else {}
        answer = {'Container.get': ['s' if shield else None]}
        case('hasDragonfireShieldProtection', ['p'], nodes=nodes, answers=answer)
        case('hasDragonfireShieldProtection(wyvern)', ['p', True], nodes=nodes, answers=answer)
    imm = {'dfi': {'kind': 'dragonfireImmunity'}, 's': item(1540), 'w': item(2890)}
    for wyvern in (False, True):
        for shield in (None, 's', 'w'):
            for potion in (None, 'dfi'):
                for prayer in (False, True):
                    case('calculateDragonfireMaxHit(all)', ['p', 50, wyvern, 3, True], nodes=imm,
                         answers={'Container.get': [shield], 'TimerManager.getTimer': [potion], 'Prayer.get': [prayer]}, sweep=(shield == 's' and potion and prayer and not wyvern))
    case('calculateDragonfireMaxHit', ['p', 50], nodes=imm, answers={'Container.get': [None], 'TimerManager.getTimer': [None], 'Prayer.get': [True]})
    case('calculateDragonfireMaxHit(all)', ['p', 50, False, 3, False], nodes=imm, answers={'Container.get': ['s'], 'TimerManager.getTimer': ['dfi'], 'Prayer.get': [True]})
    for tokens in range(16):
        case('calculateDragonfireMaxHit(all)', ['n', 61, tokens % 3 == 0, 2, True], nodes=NPC, answers={'Entity.getDragonfireProtection': [tokens]}, sweep=tokens == 15)
    case('calculateDragonfireMaxHit', ['n', 7], nodes=NPC, answers={'Entity.getDragonfireProtection': [8]})


def containers():
    case('amountInInventory', ['p', 995], answers={'Container.getAmount': [12]})
    bank = {'b1': item(1511, 4), 'b2': item(995, 100), 'b3': item(995, 7)}
    case('amountInBank', ['p', 995], nodes=bank, answers={'Container.toArray': [[None, 'b1', 'b2', 'b3'], ['b3']]}, sweep=True)
    case('amountInBank', ['p', 4], nodes=bank, answers={'Container.toArray': [['b1'], []]})
    case('amountInBank(includeSecondary)', ['p', 995, False], nodes=bank, answers={'Container.toArray': [['b2']]})
    case('amountInBank(includeSecondary)', ['p', 995, True], nodes=bank, answers={'Container.toArray': [[], ['b3', 'b2']]})
    case('amountInEquipment', ['p', 1511], nodes={'e': item(1511, 3)}, answers={'Container.get': ['e']}, sweep=True)
    case('amountInEquipment', ['p', 1511], nodes={'e': item(2572, 3)}, answers={'Container.get': ['e']})
    case('amountInEquipment', ['p', 1511], answers={'Container.get': [None]})
    case('amountInEquipment', ['p', 995], answers={'Container.get': [None]})
    case('amountInEquipment', ['p', 4242])
    case('inInventory', ['p', 995], answers={'Container.contains': [True]})
    case('inInventory(amount)', ['p', 995, 50], answers={'Container.contains': [False]})
    case('inBank', ['p', 995], nodes=bank, answers={'Container.toArray': [['b2'], []]})
    case('inBank(amount)', ['p', 995, 108], nodes=bank, answers={'Container.toArray': [['b2'], ['b3']]})
    case('inBank(amount)', ['p', 995, 108], nodes=bank, answers={'Container.toArray': [['b2'], []]})
    case('inEquipment', ['p', 1511], nodes={'e': item(1511)}, answers={'Container.get': ['e']})
    case('inEquipment(amount)', ['p', 1511, 2], nodes={'e': item(1511)}, answers={'Container.get': ['e']})
    case('anyInInventory', ['p', [1, 995, 1511]], answers={'Container.contains': [False, True, True]}, sweep=True)
    case('anyInInventory', ['p', []])
    case('inEquipmentOrInventory', ['p', 1511], nodes={'e': item(1511)}, answers={'Container.get': ['e']})
    case('inEquipmentOrInventory(amount)', ['p', 1511, 3], nodes={'e': item(1511)}, answers={'Container.get': ['e'], 'Container.contains': [True]}, sweep=True)
    case('allInEquipment', ['p', [1511, 2572]], nodes={'e': item(1511), 'r': item(2572)}, answers={'Container.get': ['e', 'r']})
    case('allInEquipment', ['p', [1511, 2572]], nodes={'e': item(1511)}, answers={'Container.get': [None, 'e']})
    case('anyInEquipment', ['p', [2572, 1511]], nodes={'e': item(1511)}, answers={'Container.get': [None, 'e']}, sweep=True)
    for slot in ('SHIELD', 'RING', 'HEAD'):
        case('getItemFromEquipment', ['p', slot], nodes={'e': item(1540)}, answers={'Container.get': ['e']})
    case('hasAnItem(id)', ['p', 995], answers={'Container.containItems': [False, True]})
    case('hasAnItem(id)', ['p', 995], answers={'Container.containItems': [False, False, True]})
    case('hasAnItem(id)', ['p', 995], nodes={'p': dict(PLAYER['p'], secondary=True)}, answers={'Container.containItems': [False, False, True]})
    case('hasAnItem(id,flags)', ['p', 995, True, False], answers={'Container.containItems': [False, False, False, True]})
    case('hasAnItem(id,flags)', ['p', 995, False, True], answers={'Container.containItems': [False, False, False], 'StorageContainer.contains': [False, False, True]}, sweep=True)
    case('hasAnItem(id,flags)', ['p', 995, True, True], answers={'Container.containItems': [False] * 4})
    case('hasAnItem(ids,flags)', ['p', [1, 995], False, False], answers={'Container.containItems': [False, False, False, True]}, sweep=True)
    case('hasAnItem(ids,flags)', ['p', [1, 995], False, True], answers={'Container.containItems': [False] * 6, 'StorageContainer.contains': [False] * 7 + [True]})
    case('ContainerisedItem.remove', ['p.inventory', 995], answers={'Container.remove': [True]}, sweep=True)
    case('ContainerisedItem.remove', [None, 995])
    gods = god_items()
    for god in gods:
        case('hasGodItem', ['p', god], answers={'Container.get': [None]})
    case('hasGodItem', ['p', 'SARADOMIN'], nodes={'e': item(gods['SARADOMIN'][2])}, answers={'Container.get': [None, None, 'e']}, sweep=True)
    case('shouldRemoveNothings', ['p'], nodes={'r': item(2572)}, answers={'Container.get': ['r']})
    case('shouldRemoveNothings', ['p'], nodes={'r': item(1511)}, answers={'Container.get': ['r']})
    case('shouldRemoveNothings', ['p'], answers={'Container.get': [None]})
    one = {'i1': item(1511, 2)}
    case('removeItem', ['p', node('i1')], nodes=one, answers={'Container.remove': [True]})
    case('removeItem', ['p', i(995)], answers={'Container.remove': [False]}, sweep=True)
    case('removeItem', ['p', None])
    case('removeItem', ['p', {'str': 'x'}])
    case('removeItem(container)', ['p', i(995), 'BANK'], answers={'Container.remove': [False, True]}, sweep=True)
    case('removeItem(container)', ['p', i(995), 'BANK'], nodes={'p': dict(PLAYER['p'], secondary=True)}, answers={'Container.remove': [True]})
    case('removeItem(container)', ['p', node('i1'), 'EQUIPMENT'], nodes=one, answers={'Container.remove': [True]})
    bob = {'bob': {'kind': 'bob', 'id': 6873}, 'fam': {'kind': 'familiar', 'id': 6830}}
    case('removeItem(container)', ['p', node('i1'), 'BoB'], nodes=dict(one, **bob), answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['bob'], 'Container.remove': [True]}, sweep=True)
    case('removeItem(container)', ['p', node('i1'), 'BoB'], nodes=dict(one, **bob), answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['fam']})
    case('removeItem(container)', ['p', node('i1'), 'BoB'], nodes=one, answers={'FamiliarManager.hasFamiliar': [False]})
    two = {'i1': item(1511, 2), 'i2': item(995, 5)}
    case('removeItemsIfPlayerHasEnough', ['p', ['i1', 'i2']], nodes=two, answers={'Container.getAmount': [1, 0]})
    case('removeItemsIfPlayerHasEnough', ['p', ['i1', 'i2']], nodes=two, answers={'Container.getAmount': [1, 3], 'Container.remove': [True, True]}, sweep=True)
    case('removeItemsIfPlayerHasEnough', ['p', ['i1', 'i2']], nodes=two, answers={'Container.getAmount': [1, 3], 'Container.remove': [False]})
    case('removeItemsIfPlayerHasEnough', ['p', []])
    case('addItem', ['p', 995], answers={'Container.add': [True]}, sweep=True)
    case('addItem(amount)', ['p', 995, 30], answers={'Container.add': [False]})
    for cont in ('BANK', 'EQUIPMENT', 'INVENTORY'):
        case('addItem(container)', ['p', 1511, 2, cont], answers={'Container.add': [True]})
    case('addItem(container)', ['p', 1511, 2, 'BANK'], nodes={'p': dict(PLAYER['p'], secondary=True)}, answers={'Container.add': [True]})
    case('addItem(container)', ['p', 1511, 2, 'BoB'], nodes=bob, answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['bob'], 'Container.add': [True]}, sweep=True)
    case('addItem(container)', ['p', 1511, 2, 'BoB'], nodes=bob, answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['fam']})
    case('addItem(container)', ['p', 1511, 2, 'BoB'], answers={'FamiliarManager.hasFamiliar': [False]})
    case('addItem', ['p', 4242])
    rs = {'new': item(1512, 5), 'cur': item(1511, 1), 'old': item(995, 1), 'gone': item(65535, 1), 'zero': item(1511, 0)}
    case('replaceSlot', ['p', 3, 'gone'], nodes=rs, answers={'Container.replace': ['old']})
    case('replaceSlot', ['p', 3, 'zero'], nodes=rs, answers={'Container.replace': [None]})
    case('replaceSlot', ['p', 3, 'new'], nodes=rs, answers={'Container.replace': ['old']}, sweep=True)
    case('replaceSlot(current)', ['p', 3, 'new', 'cur'], nodes=rs, answers={'Container.remove': [True], 'Container.replace': [None]}, sweep=True)
    case('replaceSlot(current)', ['p', 3, 'new', 'cur'], nodes=rs, answers={'Container.remove': [True], 'Container.replace': ['old']})
    case('replaceSlot(current)', ['p', 3, 'new', 'cur'], nodes=rs, answers={'Container.remove': [False, True], 'Container.replace': [None]}, sweep=True)
    case('replaceSlot(current)', ['p', 3, 'new', 'cur'], nodes=rs, answers={'Container.remove': [False, False]})
    case('replaceSlot(current)', ['p', 3, 'new', None], nodes=rs, answers={'Container.replace': ['old']})
    for cont in ('EQUIPMENT', 'BANK', 'INVENTORY'):
        case('replaceSlot(container)', ['p', 3, 'new', 'cur', cont], nodes=rs, answers={'Container.remove': [False, True], 'Container.replace': ['old']})
    case('replaceSlot(container)', ['p', 3, 'new', 'cur', 'BoB'], nodes=dict(rs, **bob), answers={'FamiliarManager.getFamiliar': ['bob'], 'Container.remove': [True], 'Container.replace': [None]})
    case('replaceSlot(container)', ['p', 3, 'new', 'cur', 'BoB'], nodes=dict(rs, **bob), answers={'FamiliarManager.getFamiliar': [None]})
    case('replaceSlot(container)', ['p', 3, 'new', 'cur', 'BoB'], nodes=dict(rs, **bob), answers={'FamiliarManager.getFamiliar': ['fam']})
    ra = {'a1': item(1511, 1, slot=2), 'a2': item(1511, 1, slot=5), 'a3': item(1511, 4, slot=0)}
    case('replaceAllItems', ['p', 1511, 2572], nodes=ra, answers={'FamiliarManager.hasFamiliar': [False], 'Container.getAll': [['a1', 'a2'], [], ['a3', 'a1'], []]}, sweep=True)
    case('replaceAllItems', ['p', 995, 1511], nodes=dict(ra, **bob), answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['bob'], 'Container.getAll': [['a1', 'a2', 'a3'], [], [], [], ['a2']]}, sweep=True)
    case('replaceAllItems', ['p', 1511, 2572], nodes=dict(ra, **bob), answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['fam'], 'Container.getAll': [[], [], [], []]})
    case('addItemOrDrop', ['p', 1511], answers={'Container.add': [False]}, sweep=True)
    case('addItemOrDrop(amount)', ['p', 995, 40], answers={'Container.add': [True]})
    case('addItemOrDrop(amount)', ['p', 1511, 3], answers={'Container.add': [True, False, True]}, sweep=True)
    case('addItemOrDrop(amount)', ['p', 1511, 0])
    case('addItemOrDrop(amount)', ['p', 1511, -2])
    case('addItemOrBank', ['p', 1511], answers={'Container.add': [True]})
    case('addItemOrBank', ['p', 1511], answers={'Container.add': [False, True], 'IronmanManager.getMode': ['NONE']}, sweep=True)
    case('addItemOrBank(amount)', ['p', 1511, 2], answers={'Container.add': [False, False, True], 'IronmanManager.getMode': ['HARDCORE']}, sweep=True)
    case('addItemOrBank(amount)', ['p', 1511, 2], answers={'Container.add': [False, False, False], 'IronmanManager.getMode': ['NONE']}, sweep=True)
    case('addItemOrBank', ['p', 1511], answers={'Container.add': [False], 'IronmanManager.getMode': ['ULTIMATE']})
    case('freeSlots', ['p'], answers={'Container.freeSlots': [9]})
    case('hasSpaceFor', ['p', 'i1'], nodes=one, answers={'Container.hasSpaceFor': [True]})
    case('removeAll', ['p', node('i1')], nodes=one, answers={'Container.getAmount': [6], 'Container.remove': [True]}, sweep=True)
    case('removeAll', ['p', i(995)], answers={'Container.getAmount': [0], 'Container.remove': [False]})
    case('removeAll', ['p', None])
    case('removeAll', ['p', {'bool': True}])
    case('removeAll(container)', ['p', i(1511), 'EQUIPMENT'], nodes={'e': item(1511, 1)}, answers={'Container.get': ['e'], 'Container.remove': [True]}, sweep=True)
    case('removeAll(container)', ['p', i(995), 'BANK'], nodes=bank, answers={'Container.toArray': [['b2'], ['b2'], ['b3']], 'Container.remove': [True, True]}, sweep=True)
    case('removeAll(container)', ['p', i(995), 'BANK'], nodes=bank, answers={'Container.toArray': [['b2'], ['b2'], ['b3']], 'Container.remove': [False]})
    case('removeAll(container)', ['p', i(995), 'INVENTORY'], answers={'Container.getAmount': [3], 'Container.remove': [True]})
    case('removeAll(container)', ['p', i(995), 'BoB'], nodes=bob, answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['bob'], 'Container.getAmount': [8], 'Container.remove': [True]}, sweep=True)
    case('removeAll(container)', ['p', i(995), 'BoB'], nodes=bob, answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['fam']})
    case('removeAll(container)', ['p', i(995), 'BoB'], answers={'FamiliarManager.hasFamiliar': [False]})
    notes = {'u': item(1511, 3, charge=7), 'nd': item(1512, 3), 'nn': item(995, 3)}
    case('note', ['u'], nodes=notes, sweep=True)
    case('note', ['nd'], nodes=notes)
    case('note', ['nn'], nodes=dict(notes, nn=item(229)))
    case('unnote', ['u'], nodes=notes)
    case('unnote', ['nd'], nodes=notes, sweep=True)
    case('unnote', ['nn'], nodes=notes)
    for eid in (1540, 2572, 1, 4242):
        case('equipSlot', [eid])
    case('hasSealOfPassage', ['p'], nodes={'e': item(9083)}, answers={'Container.get': ['e']})
    case('hasSealOfPassage', ['p'], answers={'Container.get': [None], 'Container.contains': [True]})
    dump = {'d1': item(1511, 1), 'd2': item(1512, 4), 'd3': item(995, 9)}
    case('dumpContainer', ['p', 'p.inventory'], nodes=dump, answers={'IronmanManager.isIronman': [False], 'Container.toArray': [['d1', None, 'd2', 'd3']], 'Container.hasSpaceFor': [True, True, True], 'Container.canAdd': [True, False, True], 'Container.remove': [True, True], 'Container.add': [True, True]}, sweep=True)
    case('dumpContainer', ['p', 'p.equipment'], nodes=dump, answers={'IronmanManager.isIronman': [True], 'IronmanManager.getMode': ['STANDARD'], 'Container.toArray': [['d1', 'd2', 'd3']], 'Container.hasSpaceFor': [True, True, True], 'Container.canAdd': [True], 'InteractionListeners.run': [False, True], 'SlayerEquipmentFlags.isSlayerEq': [True, False]}, sweep=True)
    case('dumpContainer', ['p', 'p.inventory'], nodes=dump, answers={'IronmanManager.isIronman': [False], 'Container.toArray': [['d1', 'd2']], 'Container.hasSpaceFor': [True, False], 'Container.canAdd': [True]})
    case('dumpContainer', ['p', 'p.inventory'], nodes=dump, answers={'IronmanManager.isIronman': [True], 'IronmanManager.getMode': ['ULTIMATE']})
    case('dumpBeastOfBurden', ['p'], answers={'FamiliarManager.hasFamiliar': [False]})
    case('dumpBeastOfBurden', ['p'], nodes=bob, answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['fam']})
    case('dumpBeastOfBurden', ['p'], nodes=bob, answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['bob'], 'Container.isEmpty': [True]})
    for count, canadd in ((2, [True, True]), (3, [True, True]), (4, [True, True]), (2, [False, False])):
        case('dumpBeastOfBurden', ['p'], nodes=dict(bob, **dump), answers={'FamiliarManager.hasFamiliar': [True], 'FamiliarManager.getFamiliar': ['bob'], 'Container.isEmpty': [False], 'Container.itemCount': [count], 'IronmanManager.isIronman': [False], 'Container.toArray': [['d1', 'd2']], 'Container.hasSpaceFor': [True, True], 'Container.canAdd': canadd}, sweep=count == 3)
    pots = {'a4': item(2428, 1), 'a3': item(121, 1), 'a2': item(123, 2), 's1': item(119, 1), 's3': item(115, 1), 'xx': item(1511, 1),
            'attack': {'kind': 'potion', 'ids': [2428, 121, 123, 125], 'hash': 7}, 'strength': {'kind': 'potion', 'ids': [113, 115, 117, 119], 'hash': 3},
            'food': {'kind': 'consumable', 'ids': [1511]}}
    case('decantContainer', ['p.inventory'], nodes=pots, answers={'Container.toArray': [['a3', None, 'a2', 's1', 'xx', 's3', 'a4']], 'Consumables.getConsumableById': ['attack', 'attack', 'strength', 'food', 'strength', 'attack']}, sweep=True)
    case('decantContainer', ['p.inventory'], nodes=dict(pots, strength=dict(pots['strength'], hash=23)), answers={'Container.toArray': [['a3', 'a2', 's1', 's3']], 'Consumables.getConsumableById': ['attack', 'attack', 'strength', 'strength']})
    case('decantContainer', ['p.inventory'], nodes=dict(pots, odd=item(1511, 1)), answers={'Container.toArray': [['a3', 'xx']], 'Consumables.getConsumableById': ['attack', 'attack']})
    case('decantContainer', ['p.inventory'], nodes=pots, answers={'Container.toArray': [[]]})


def visuals():
    case('getAnimation', [866])
    case('getAnimationWithPriority', [866, 'HIGH'])
    case('resetAnimator', ['p'], sweep=True)
    case('renderAnimation', ['p', 1426], sweep=True)
    case('resetRenderAnimation', ['p'], sweep=True)
    case('animationDuration', [{'anim': [866, 'LOW']}], answers={'AnimationDefinition.getDurationTicks': [4]})
    case('animationCycles', [866], answers={'AnimationDefinition.getCycles': [38]})
    case('animate', ['p', i(866)], answers={'Animator.animate': [True]})
    case('animate', ['n', {'anim': [866, 'HIGH']}], nodes=NPC)
    case('animate', ['p', {'str': '866'}])
    case('animate(forced)', ['p', i(866), True])
    case('animate(forced)', ['p', i(866), False])
    case('visualize', ['p', i(866), i(110)], answers={'Entity.visualize': [True]})
    case('visualize', ['p', {'anim': [866, 'VERY_HIGH']}, {'gfx': [110, 96, 2]}])
    case('visualize', ['p', None, i(110)])
    case('visualize', ['p', i(866), {'str': 'x'}])
    case('sendGraphics', [i(110), loc(1, 2, 3)])
    case('sendGraphics', [{'gfx': [110, 1, 2]}, loc(1, 2)])
    case('sendGraphics', [{'str': 'x'}, loc(1, 2)])
    case('emote', ['p', 'BOW'])
    sc = {'s': {'kind': 'scenery', 'id': 1530, 'location': loc(3210, 3211, 1), 'rotation': 2}}
    case('animateScenery(player)', ['p', 's', 4135], nodes=sc)
    case('animateScenery(player,global)', ['p', 's', 4135, True], nodes=sc)
    case('animateScenery', ['s', 4135], nodes=sc, sweep=True)
    case('animateInterface', ['p', 9, 3, 7])
    case('spawnProjectile(entity)', ['p', 'n', 88], nodes=dict(NPC, proj={'kind': 'projectile'}), answers={'Projectile.create': ['proj']}, sweep=True)
    case('spawnProjectile(entity)', ['p', 'n', 88], nodes=NPC, answers={'Projectile.create': [None]})
    case('spawnProjectile(location)', [loc(3200, 3200), loc(3207, 3204), 88, 40, 30, 5, 60, 16], nodes={'proj': {'kind': 'projectile'}}, answers={'Projectile.create': ['proj']}, sweep=True)
    case('playJingle', ['p', 151], sweep=True)
    case('playAudio', ['p', 2727])
    case('playAudio(delay,loops)', ['p', 2727, 20, 3])
    case('playAudio(all)', ['p', 2727, 20, 3, loc(5, 6, 1), 4])
    case('playAudio(all)', ['p', 2727, 0, 1, None, 8])
    case('playGlobalAudio', [loc(3200, 3200), 2727], nodes=P2, answers={'RegionManager.getLocalPlayers': [['p', 'p2']]}, sweep=True)
    case('playGlobalAudio(all)', [loc(3200, 3200), 2727, 5, 2, 12], answers={'RegionManager.getLocalPlayers': [[]]})
    for s in (532, 7, 99, 1234567):
        case('playHurtAudio', ['p'], seed=s)
        case('playHurtAudio(delay)', ['p2', 15], nodes=P2, seed=s)
    case('impact', ['p', 5], sweep=True, seed=11)
    case('impact', ['n', 5], nodes=NPC)
    case('impact(type)', ['p2', 3, 'POISON'], nodes=P2, seed=12)


def scenery():
    sc = {'s': {'kind': 'scenery', 'id': 1530, 'location': loc(3210, 3211, 1), 'rotation': 2}, 's2': {'kind': 'scenery', 'id': 1531, 'location': loc(3210, 3211, 1)}}
    case('replaceScenery', ['s', 1531, -1], nodes=sc, answers={'Scenery.transform': ['s2']}, sweep=True)
    case('replaceScenery', ['s', 1531, 50], nodes=sc, answers={'Scenery.transform': ['s2']})
    case('replaceScenery(location)', ['s', 1531, 50, loc(1, 1)], nodes=sc, answers={'Scenery.transform': ['s2']}, sweep=True)
    case('replaceScenery(location)', ['s', 1531, -1, None], nodes=sc, answers={'Scenery.transform': [None]})
    for d in ('NORTH_WEST', 'NORTH', 'NORTH_EAST', 'EAST', 'SOUTH_EAST', 'SOUTH', 'SOUTH_WEST', 'WEST'):
        case('replaceScenery(direction)', ['s', 1531, -1, d], nodes=sc, answers={'Scenery.transform': ['s2']})
    case('replaceScenery(direction,location)', ['s', 1531, 9, 'EAST', loc(2, 2)], nodes=sc, answers={'Scenery.transform': ['s2']}, sweep=True)
    case('addScenery(id)', [1530, loc(1, 2)], sweep=True)
    case('addScenery(id,rotation,type)', [1530, loc(1, 2), 3, 10])
    case('addScenery(scenery)', ['s'], nodes=sc)
    case('removeScenery', ['s'], nodes=sc, answers={'SceneryBuilder.remove': [True]})
    case('getScenery(xyz)', [1, 2, 3], nodes=sc, answers={'RegionManager.getObject': ['s']})
    case('getScenery(location)', [loc(1, 2, 3)], answers={'RegionManager.getObject': [None]})
    case('getChildren', [1530], answers={'SceneryDefinition.forId': ['def'], 'SceneryDefinition.getChildrenIds': [[-1, 5, -1, 7]]})
    case('getChildren', [1530], answers={'SceneryDefinition.forId': ['def'], 'SceneryDefinition.getChildrenIds': [None]})
    case('getChildren', [1530], answers={'SceneryDefinition.forId': [None]})
    case('sceneryDefinition', [1530], answers={'SceneryDefinition.forId': ['def']})
    case('sceneryDefinition', [1530], answers={'SceneryDefinition.forId': [None]})
    case('getItemName', [1511])
    case('getItemName', [4242])
    case('getItemName', [65535])
    case('itemDefinition', [995])
    case('itemDefinition', [4242])
    case('hasOption', ['n', 'Talk-to'], nodes=NPC, answers={'NPCDefinition.hasAction': [True]})
    case('hasOption', ['s', 'Open'], nodes=sc, answers={'SceneryDefinition.forId': ['def'], 'SceneryDefinition.hasAction': [False]})
    case('hasOption', ['i1', 'Eat'], nodes={'i1': item(1511)}, answers={'ItemDefinition.hasAction': [True]})
    case('hasOption', ['p', 'Trade'])
    g = {'g': {'kind': 'grounditem', 'id': 995, 'amount': 3}}
    case('removeGroundItem', ['g'], nodes=g)
    case('isValidGroundItem', ['g'], nodes=g, answers={'GroundItemManager.getItems': [['g']]})
    case('isValidGroundItem', ['g'], nodes=g, answers={'GroundItemManager.getItems': [[]]})
    case('produceGroundItem(player)', ['p', 995], answers={'GroundItemManager.create': ['g']}, nodes=g, sweep=True)
    case('produceGroundItem(owner)', [None, 995, 7, loc(3, 4, 1)], nodes=g, answers={'GroundItemManager.create': ['g']}, sweep=True)
    case('produceGroundItem(owner)', ['p', 995, 7, loc(3, 4, 1)], nodes=g, answers={'GroundItemManager.create': [None]})
    ch = dict(sc, i1=item(1511, charge=10))
    for target in ('i1', 's', 'p'):
        case('adjustCharge', [target, 5], nodes=ch, sweep=target == 'i1')
        case('getCharge', [target], nodes=ch)
        case('setCharge', [target, 77], nodes=ch)
    case('adjustCharge', ['i1', 70000], nodes=ch)


def world():
    case('getWorldTicks', [], world={'ticks': 77})
    case('poofClear', ['n'], nodes=NPC, answers={'Region.isActive': [True]}, steps=[{'step': 'pulse', 'index': 0}] * 4, sweep=True)
    case('poofClear', ['n'], nodes=NPC, answers={'Region.isActive': [False]}, steps=[{'step': 'pulse', 'index': 0}] * 4)
    case('findNPC(id)', [1], nodes=NPC, answers={'Repository.findNPC': ['n']})
    case('findNPC(id)', [1], answers={'Repository.findNPC': [None]})
    many = {'n': {'kind': 'npc', 'id': 1, 'location': loc(3300, 3300)}, 'm': {'kind': 'npc', 'id': 2, 'location': loc(3201, 3201)},
            'k': {'kind': 'npc', 'id': 1, 'location': loc(3205, 3212)}, 'z': {'kind': 'npc', 'id': 1, 'location': loc(3201, 3201, 1)}}
    case('findNPC(location)', [loc(3200, 3200), 1], nodes=many, answers={'Repository.getNpcs': [['n', 'm', 'z', 'k']]}, sweep=True)
    case('findNPC(location)', [loc(3200, 3200), 1], nodes=many, answers={'Repository.getNpcs': [['n', 'z']]})
    case('findLocalNPC', ['p', 1], nodes=many, answers={'RegionManager.getLocalNPCs': [['m', 'k', 'n']]})
    case('findLocalNPC', ['p', 9], nodes=many, answers={'RegionManager.getLocalNPCs': [['m']]})
    case('findLocalNPCs(location)', [loc(1, 2), 4], nodes=many, answers={'RegionManager.getLocalNPCs': [['m', 'k']]})
    case('findLocalNPCs(ids)', ['p', [1, 3]], nodes=many, answers={'RegionManager.getLocalNPCs': [['n', 'm', 'k']]})
    case('findLocalNPCs(ids,distance)', ['p', [2], 3], nodes=many, answers={'RegionManager.getLocalNPCs': [['n', 'm', 'k']]})
    case('getRegionBorders', [12850], answers={'ZoneBorders.forRegion': [[3200, 3200, 3263, 3263, 0]]})
    for xyz in ((3200, 3200, 0), (5, -7, 2), (17000, 3, 5), (1, 1, -1), (-16385, 16384, 3)):
        case('location', list(xyz))
    case('inBorders(borders)', ['p', [1, 2, 3, 4, 0]], answers={'ZoneBorders.insideBorder': [True]})
    case('inBorders(corners)', ['n', 1, 2, 3, 4], nodes=NPC, answers={'ZoneBorders.insideBorder': [False]})
    case('transformNpc', ['n', 50, 10], nodes=NPC, steps=[{'step': 'pulse', 'index': 0}], sweep=True)
    s = {'act': {'kind': 'script', 'persist': False, 'nextExecution': 3}}
    case('transformNpcWithHpCarryover', ['n', 50, 10], nodes=dict(NPC, **s), world={'ticks': 40}, answers={'ScriptProcessor.getActiveScript': ['act']},
         steps=[{'step': 'script', 'index': 0, 'stage': 0}, {'step': 'script', 'index': 0, 'stage': 1}], sweep=True)
    case('transformNpcWithHpCarryover', ['n', 50, 10], nodes=dict(NPC, **s), answers={'ScriptProcessor.getActiveScript': [None]},
         steps=[{'step': 'script', 'index': 0, 'stage': 0}, {'step': 'script', 'index': 0, 'stage': 7}])
    case('forceWalk', ['p', loc(3205, 3200), 'clip'], sweep=True)
    case('forceWalk', ['p', loc(3205, 3200), 'smart'], answers={'Pathfinder.find': [{'successful': True}]}, sweep=True)
    case('forceWalk', ['n', loc(3205, 3200), 'DUMB'], nodes=NPC, answers={'Pathfinder.find': [{'successful': False}]})
    case('forceWalk', ['n', loc(3205, 3200), 'dumb'], nodes=NPC, answers={'Pathfinder.find': [None]})
    for who, dest in (('p', loc(3250, 3240, 1)), ('p', loc(3203, 3204)), ('n', loc(3240, 3170)), ('p', loc(3200, 3200)), ('n', loc(3201, 3215, 2)), ('p', loc(3190, 3150))):
        case('truncateLoc', [who, dest], nodes=NPC)
    case('truncateLoc', ['p', loc(3250, 3240)], world={'maxPathfind': 7})
    fm = [{'step': 'ticks', 'ticks': 100}, {'step': 'script', 'index': 0, 'stage': 0}, {'step': 'script', 'index': 0, 'stage': 1},
          {'step': 'ticks', 'ticks': 103}, {'step': 'script', 'index': 0, 'stage': 2}, {'step': 'ticks', 'ticks': 105}, {'step': 'script', 'index': 0, 'stage': 3},
          {'step': 'ticks', 'ticks': 109}, {'step': 'script', 'index': 0, 'stage': 4}]
    fmn = {'p': dict(PLAYER['p'], clocks=[100, 104, 0, 0, 0, 0, 0, 0])}
    case('forceMove', ['p', loc(3200, 3200), loc(3200, 3206), 30, 60], nodes=fmn, world={'ticks': 100}, steps=fm, answers={'ScriptProcessor.getActiveScript': [None]}, sweep=True)
    case('forceMove', ['p', loc(3200, 3200), loc(3195, 3197), 31, 91], nodes=fmn, world={'ticks': 100}, steps=fm)
    case('forceMove', ['p', loc(3200, 3200), loc(3203, 3203), 0, 0], world={'ticks': 100}, steps=fm)
    case('forceMove(all)', ['p', loc(3200, 3200), loc(3210, 3200), 45, 30, 'SOUTH', 4321, 'done'], nodes=fmn, world={'ticks': 100}, steps=fm, sweep=True)
    case('forceMove(all)', ['p', loc(3200, 3200), loc(3210, 3200), 45, 30, None, 4321, None], world={'ticks': 100}, steps=fm)
    case('stopWalk', ['n'], nodes=NPC, sweep=True)
    case('flee', ['n', 'p'], nodes=NPC, answers={'Pathfinder.find': [{'successful': True}]}, sweep=True)
    case('teleport', ['p', loc(1, 2, 3)], sweep=True)
    case('teleport(type)', ['p', loc(1, 2, 3), 'NORMAL'], answers={'TeleportManager.send': [True]}, sweep=True)
    case('teleport(type)', ['n', loc(1, 2, 3), 'INSTANT'], nodes=NPC)
    case('face', ['p', {'loc': [1, 2, 0]}] if False else ['p', 'n'], nodes=NPC)
    case('face', ['n', 'g'], nodes=dict(NPC, g={'kind': 'grounditem', 'id': 995}))
    case('face(duration)', ['p', 'n', 4], nodes=NPC, sweep=True)
    case('face(duration)', ['p', 'n', -1], nodes=NPC)
    case('face(duration)', ['p', 'g', 4], nodes=dict(NPC, g={'kind': 'grounditem', 'id': 995}))
    case('resetFace', ['p'], sweep=True)
    rl = {'p': dict(PLAYER['p'], size=2)}
    case('getPathableRandomLocalCoordinate', ['p', 5, loc(3200, 3200, 1)], nodes=rl, answers={'ZoneBorders.getRandomLoc': [loc(3201, 3203, 1)], 'Pathfinder.find': [{'successful': True}]}, sweep=True)
    case('getPathableRandomLocalCoordinate', ['p', 40, loc(3200, 3200, 1)], nodes=rl, answers={'ZoneBorders.getRandomLoc': [loc(3201, 3203, 1), loc(3202, 3203, 1), loc(3203, 3203, 1)], 'Pathfinder.find': [{'successful': False}, {'successful': True, 'moveNear': True}, {'successful': False}]})
    case('getPathableRandomLocalCoordinate(attempts)', ['p', 2, loc(3200, 3200, 1), 1], nodes=rl, answers={'ZoneBorders.getRandomLoc': [loc(3201, 3203, 1)], 'Pathfinder.find': [{'successful': False}]})
    case('getPathableRandomLocalCoordinate(attempts)', ['p', 2, loc(3200, 3200, 1), 0], nodes=rl)
    case('getPathableRandomLocalCoordinate', ['p', 3, loc(3200, 3200, 1)], world={'maxPathfind': 3}, nodes=rl, answers={'ZoneBorders.getRandomLoc': [loc(3201, 3201, 1)], 'Pathfinder.find': [{'successful': True}]})
    case('getPathableCardinal', ['p', loc(3200, 3200)], answers={'Pathfinder.find': [{'successful': False}, {'successful': True, 'moveNear': True}, {'successful': True}]}, sweep=True)
    case('getPathableCardinal', ['p', loc(3200, 3200)], answers={'Pathfinder.find': [{'successful': False}]})
    case('registerMapZone', ['zone', [1, 2, 3, 4, 0]], sweep=True)
    case('addClimbDest', [loc(1, 2), loc(3, 4, 1)])
    door = {'d': {'kind': 'scenery', 'id': 1530, 'location': loc(3201, 3200)}}
    case('handleDoor', ['p', 'd'], nodes=door)
    case('handleDoor', ['n', 'd'], nodes=dict(NPC, **door))
    case('handleAutowalkDoor', ['p', 'd'], nodes=door, answers={'DoorActionHandler.getEndLocation': [loc(3202, 3200)], 'DoorActionHandler.handleAutowalkDoorWithLocation': [True]}, sweep=True)
    case('handleAutowalkDoor', [None, None], answers={'DoorActionHandler.getEndLocation': [None]})
    case('hasLineOfSight', ['p', 'n'], nodes=NPC, answers={'CombatSwingHandler.isProjectileClipped': [True]})
    case('isPlayer', ['p'])
    case('isPlayer', ['n'], nodes=NPC)


def interfaces():
    for fn, args in (('openInterface', ['p', 300]), ('openOverlay', ['p', 301]), ('closeOverlay', ['p']), ('openSingleTab', ['p', 302]),
                     ('closeInterface', ['p']), ('closeTabInterface', ['p']), ('closeAllInterfaces', ['p']), ('setInterfaceText', ['p', 'hi', 3, 4]),
                     ('sendString', ['p', 'hi', 3, 4]), ('setComponentVisibility', ['p', 3, 4, True]), ('restoreTabs', ['p']),
                     ('sendPlayerOnInterface', ['p', 3, 4]), ('sendAnimationOnInterface', ['p', 9, 3, 4]), ('sendItemOnInterface', ['p', 3, 4, 995]),
                     ('sendItemOnInterface(amount)', ['p', 3, 4, 995, 7]), ('sendItemZoomOnInterface', ['p', 3, 4, 995]),
                     ('sendItemZoomOnInterface(zoom)', ['p', 3, 4, 995, 90]), ('closeDialogue', ['p'])):
        case(fn, args, sweep=fn in ('closeAllInterfaces', 'closeDialogue'))
    for options in (1, 2, 3, 4, 5, 6):
        case('setTitle', ['p', options], sweep=options == 6)
    case('removeTabs', ['p', ['COMBAT', 'MUSIC', 'AME_DEFAULT']])
    case('removeTabs', ['p', []])
    case('sendMessage', ['p', 'Short message.'], sweep=True)
    case('sendMessage', ['p', ' '.join(['word'] * 40)])
    case('sendMessage', ['p', 'x' * 86])
    case('sendMessage', ['p', 'y' * 40 + ' ' + 'z' * 45])
    case('sendDialogue', ['p', 'x' * 54])
    case('sendMessage', ['p', '<col=ff0000>' + 'red text continues ' * 6 + '</col> done'])
    case('sendChat', ['n', 'Hello!'], nodes=NPC)
    case('sendChat(delay)', ['n', 'Later!', -1], nodes=NPC)
    act = {'act': {'kind': 'script', 'persist': True}}
    case('sendChat(delay)', ['n', 'Later!', 3], nodes=dict(NPC, **act), world={'ticks': 9}, answers={'ScriptProcessor.getActiveScript': [None]}, steps=[{'step': 'script', 'index': 0, 'stage': 0}], sweep=True)
    case('sendChat(delay)', ['n', 'Later!', 0], nodes=dict(NPC, **act), answers={'ScriptProcessor.getActiveScript': ['act']}, steps=[{'step': 'script', 'index': 0, 'stage': 0}], sweep=True)
    case('sendNews', ['Something happened.'])
    case('sendDialogue', ['p', 'A short line.'])
    case('sendDialogue', ['p', ' '.join(['longword'] * 30)], sweep=True)
    case('sendDialogueLines', ['p', ['one', 'two']])
    case('sendDialogueOptions', ['p', 'Pick', ['a', 'b', 'c']])
    case('sendPlayerDialogue', ['p', 'Hi there.'])
    case('sendPlayerDialogue(all)', ['p', ' '.join(['talk'] * 20), 'HAPPY', True])
    case('sendNPCDialogue', ['p', 1, 'Hello.'])
    case('sendNPCDialogue(all)', ['p', 1, 'Hello.', 'ANGRY', True])
    case('sendNPCDialogueLines', ['p', 1, 'ASKING', True, ['a', 'b', 'c']], answers={'DialogueInterpreter.sendDialogues': [241]}, sweep=True)
    case('sendNPCDialogueLines', ['p', 1, 'ASKING', False, []], answers={'DialogueInterpreter.sendDialogues': [None]})
    case('sendItemDialogue', ['p', node('i1'), 'Look!'], nodes={'i1': item(1511)})
    case('sendItemDialogue', ['p', i(995), 'Look at all this money!'], sweep=True)
    case('sendItemDialogue', ['p', {'str': 'x'}, 'Nope'])
    case('sendDoubleItemDialogue', ['p', node('i1'), node('i2'), 'Two'], nodes={'i1': item(1511), 'i2': item(995)})
    case('sendDoubleItemDialogue', ['p', i(1511), i(995), 'Two'])
    case('sendDoubleItemDialogue', ['p', node('i1'), i(995), 'Two'], nodes={'i1': item(1511)})
    case('sendDoubleItemDialogue', ['p', i(1511), node('i1'), 'Two'], nodes={'i1': item(1511)})
    case('sendDoubleItemDialogue', ['p', {'str': 'a'}, i(1), 'Two'])
    case('sendInputDialogue(numeric)', ['p', True, 'How many?', 'h'], sweep=True)
    case('sendInputDialogue(numeric)', ['p', False, 'Name?', 'h'])
    for t in ('AMOUNT', 'NUMERIC', 'STRING_SHORT', 'STRING_LONG', 'MESSAGE'):
        case('sendInputDialogue(type)', ['p', t, 'Enter:', 'h'], sweep=t == 'AMOUNT')
    case('openDialogue', ['p', i(5), [i(1), {'str': 'x'}]], sweep=True)
    case('openDialogue', ['p', {'dialogueFile': 'F'}, []], sweep=True)
    case('openDialogue', ['p', {'str': 'nope'}, []])
    case('openDialogue', ['p', {'bool': True}, []])
    case('addDialogueAction', ['p', 'act'])
    case('runcs2', ['p', 150, [i(1), {'str': 'a'}, i(3)]], sweep=True)
    case('runcs2', ['p', 150, [i(1), {'bool': True}, i(3)]])
    case('runcs2', ['p', 150, []])
    case('sendItemSelect', ['p', ['Eat', 'Drop'], 'cb'], sweep=True)
    case('sendItemSelect(keepAlive)', ['p', [f'o{k}' for k in range(11)], True, 'cb'])
    for count in (0, 1, 2, 3, 4, 5, 6):
        spec = {'ids': [995 + k for k in range(count)][:count]} if count else {}
        if count in (1, 2):
            spec = {'items': [f'k{k}' for k in range(count)], 'create': True, 'max': True}
        nodes = {f'k{k}': item(1511 + k % 2) for k in range(2)}
        steps = [{'step': 'create', 'amount': 5, 'index': count - 1}, {'step': 'getAll', 'index': 0}] if count in (1, 2, 3) else None
        defs = {str(995 + k): {'name': f'Thing {k}'} for k in range(6)}
        case('sendSkillDialogue', ['p', spec], nodes=nodes, steps=steps, answers={'@lambda': [7], 'Container.getAmount': [12]}, sweep=count == 3)
        ITEMS.update({k: v for k, v in defs.items() if k not in ITEMS})
    case('registerLogoutListener', ['p', 'k', 'h'], nodes={'p': dict(PLAYER['p'], listeners=['a'])})
    case('clearLogoutListener', ['p', 'a'], nodes={'p': dict(PLAYER['p'], listeners=['a', 'b'])})
    case('clearLogoutListener', ['p', 'zz'])
    case('registerHintIcon(location)', ['p', loc(1, 2), 30], answers={'HintIconManager.freeSlot': [2], 'HintIconManager.registerHintIcon': [2]}, sweep=True)
    case('registerHintIcon(node)', ['p', 'n'], nodes=NPC, answers={'Entity.getAttribute': [None], 'HintIconManager.registerHintIcon': [4]}, sweep=True)
    case('registerHintIcon(node)', ['p', 'n'], nodes=NPC, answers={'Entity.getAttribute': [1]})
    case('clearHintIcon', ['p'], answers={'Entity.getAttribute': [3]}, sweep=True)
    case('clearHintIcon', ['p'], answers={'Entity.getAttribute': [None]})
    case('clearHintIcon', ['p'], answers={'Entity.getAttribute': ['three']})


def attributes_and_timers():
    case('getAttribute', ['p', 'k', i(4)], answers={'Entity.getAttribute': [None]})
    case('getAttribute', ['p', 'k', {'str': 'd'}], answers={'Entity.getAttribute': ['stored']})
    case('getAttribute', ['n', '/save:k', None], nodes=NPC, answers={'Entity.getAttribute': [{'node': 'p'}]})
    case('setAttribute', ['p', '/save:k', i(4)])
    case('setAttribute', ['p', 'k', None])
    case('setAttribute', ['p', 'k', {'loc': [1, 2, 3]}])
    case('removeAttribute', ['p', '/save:k'])
    case('removeAttribute', ['p', 'a/save:b/save:'])
    case('removeAttributes', ['p', ['/save:a', 'b']], sweep=True)
    tm = {'poi': {'kind': 'poison', 'severity': 10, 'source': 'p'}, 'imm': {'kind': 'poisonImmunity'}, 'dfi': {'kind': 'dragonfireImmunity'}}
    case('registerTimer', ['p', 'poi'], nodes=tm)
    case('registerTimer', ['p', None])
    case('getOrStartTimer', ['p', 'poison', [i(1)]], nodes=tm, answers={'TimerManager.getTimer': ['poi']})
    case('getOrStartTimer', ['p', 'poison', [{'node': 'p'}, i(4)]], nodes=tm, answers={'TimerManager.getTimer': [None], 'TimerRegistry.getTimerInstance': ['poi']}, sweep=True)
    case('getOrStartTimer', ['p', 'nothing', []], answers={'TimerManager.getTimer': [None], 'TimerRegistry.getTimerInstance': [None]})
    case('getOrStartTimer<Poison>', ['p', [{'node': 'p'}, i(4)]], nodes=tm, answers={'TimerManager.getTimer': [None], 'TimerRegistry.getTimerInstance': ['poi']}, sweep=True)
    case('getOrStartTimer<Poison>', ['p', []], nodes=tm, answers={'TimerManager.getTimer': ['poi']})
    case('getOrStartTimer<Poison>', ['p', []], nodes=tm, answers={'TimerManager.getTimer': [None], 'TimerRegistry.getTimerInstance': [None]})
    case('spawnTimer', ['poison', [i(3)]], nodes=tm, answers={'TimerRegistry.getTimerInstance': ['poi']})
    case('spawnTimer<Poison>', [[i(3)]], nodes=tm, answers={'TimerRegistry.getTimerInstance': [None]})
    case('spawnTimer<Poison>', [[]], nodes=tm, answers={'TimerRegistry.getTimerInstance': ['poi']})
    case('hasTimerActive<Poison>', ['p'], nodes=tm, answers={'TimerManager.getTimer': ['poi']})
    case('getTimer<Poison>', ['p'], nodes=tm, answers={'TimerManager.getTimer': [None]})
    case('getTimer<Poison>', ['p'], nodes=tm, answers={'TimerManager.getTimer': ['imm']})
    case('removeTimer<Poison>', ['p'])
    case('hasTimerActive', ['p', 'poison'], nodes=tm, answers={'TimerManager.getTimer': ['poi']})
    case('getTimer', ['p', 'poison'], answers={'TimerManager.getTimer': [None]})
    case('removeTimer(identifier)', ['p', 'poison'])
    case('removeTimer(timer)', ['p', 'poi'], nodes=tm)
    case('applyPoison', ['p', 'n', 6], nodes=dict(NPC, **tm), answers={'TimerManager.getTimer': ['imm']})
    case('applyPoison', ['p', 'n', 6], nodes=dict(NPC, **tm), answers={'TimerManager.getTimer': [None], 'Entity.isPoisonImmune': [True]})
    case('applyPoison', ['p', 'n', 6], nodes=dict(NPC, **tm), answers={'TimerManager.getTimer': [None, 'poi']}, sweep=True)
    case('applyPoison', ['p', 'n', 10], nodes=dict(NPC, **tm), answers={'TimerManager.getTimer': [None, 'poi']})
    case('applyPoison', ['p', 'n', 12], nodes=dict(NPC, **tm), answers={'TimerManager.getTimer': [None, 'poi']}, sweep=True)
    case('applyPoison', ['n', 'p', 12], nodes=dict(NPC, **tm), answers={'TimerManager.getTimer': [None, None], 'TimerRegistry.getTimerInstance': ['poi']}, sweep=True)
    case('applyPoison', ['n', 'p', 12], nodes=dict(NPC, **tm), answers={'TimerManager.getTimer': [None, None], 'TimerRegistry.getTimerInstance': [None]})
    case('isPoisoned', ['p'], nodes=tm, answers={'TimerManager.getTimer': ['poi']})
    case('curePoison', ['p'], nodes=tm, answers={'TimerManager.getTimer': ['poi']}, sweep=True)
    case('curePoison', ['n'], nodes=dict(NPC, **tm), answers={'TimerManager.getTimer': ['poi']})
    case('curePoison', ['p'], answers={'TimerManager.getTimer': [None]})
    case('lock', ['p', 3])
    case('lockInteractions', ['p', 3])
    case('unlock', ['p'])


def varps():
    pv = {'p': dict(PLAYER['p'], varps={'5': 77, '21': 3, '37': -1}, saved={'21': True, '37': False})}
    vb = {'100': [21, 4, 7], '101': [5, 0, 31], '102': [999, 30, 31]}
    case('getVarp', ['p', 5], nodes=pv)
    case('getVarp', ['p', 6], nodes=pv)
    case('getVarbit', ['p', 100], nodes=pv, varbits=vb)
    case('getVarbit', ['p', 102], nodes=pv, varbits=vb)
    case('getVarbit', ['p', 4242], nodes=pv, varbits=vb)
    case('getVarbit(definition)', ['p', 101], nodes=pv, varbits=vb)
    case('setVarp', ['p', 5, 9], nodes=pv, sweep=True)
    case('setVarp(save)', ['p', 5, 9, True], nodes=pv)
    case('setVarp(save)', ['p', 37, 9, True], nodes=pv)
    case('setVarp(save)', ['p', 21, 9, False], nodes=pv)
    case('saveVarp', ['p', 6], nodes=pv)
    case('unsaveVarp', ['p', 21], nodes=pv)
    case('setVarbit', ['p', 100, 0x1F], nodes=pv, varbits=vb, sweep=True)
    case('setVarbit(save)', ['p', 102, 3, True], nodes=pv, varbits=vb)
    case('setVarbit', ['p', 4242, 1], nodes=pv, varbits=vb)
    case('setVarbit(definition)', ['p', 101, -5, False], nodes=pv, varbits=vb)
    case('setVarc', ['p', 40000, 3], sweep=True)
    case('reinitVarps', ['p'], nodes={'p': dict(PLAYER['p'], varps={'1': 4, '17': 5, '3': 6, '64': 1}, saved={'17': True})}, sweep=True)
    case('reinitVarps', ['p'])


def scripts():
    case('getUsedOption', ['p'], answers={'Entity.getAttribute': ['eat']})
    case('getUsedOption', ['p'], answers={'Entity.getAttribute': [None]})
    case('getUsedOption', ['p'], answers={'Entity.getAttribute': [4]})
    case('submitWorldPulse', ['work', 3], sweep=True)
    case('runWorldTask', ['t'], steps=[{'step': 'pulse', 'index': 0}], sweep=True)
    case('submitIndividualPulse', ['p', 'work', 2])
    case('runTask', ['p', 't'], steps=[{'step': 'pulse', 'index': 0}], sweep=True)
    case('runTask(delay,repeat)', ['p', 3, 2, 't'], steps=[{'step': 'pulse', 'index': 0}] * 3)
    case('runTask(delay,repeat)', ['p', 0, 0, 't'], steps=[{'step': 'pulse', 'index': 0}])
    clocks = {'p': dict(PLAYER['p'], clocks=[10, 12, 0, 0, 0, 3, 15, 0])}
    for t in (9, 10, 11, 12, 15, 16):
        case('finishedMoving', ['p'], nodes=clocks, world={'ticks': t})
        case('animationFinished', ['p'], nodes=clocks, world={'ticks': t})
        case('isStunned', ['p'], nodes=clocks, world={'ticks': t})
    act = {'act': {'kind': 'script', 'persist': False, 'nextExecution': 1, 'state': 4}, 'per': {'kind': 'script', 'persist': True}}
    case('delayScript', ['p', 5], nodes=act, world={'ticks': 20}, answers={'ScriptProcessor.getActiveScript': ['act']})
    case('delayScript', ['p', 5], nodes=act, world={'ticks': 20}, answers={'ScriptProcessor.getActiveScript': [None]})
    case('delayEntity', ['p', 4], world={'ticks': 20}, sweep=True)
    case('apRange', ['p', 3], sweep=True)
    case('clearScripts', ['p'])
    case('restartScript', ['p'], nodes=act, answers={'ScriptProcessor.getActiveScript': ['per']})
    case('restartScript', ['p'], nodes=act, answers={'ScriptProcessor.getActiveScript': ['act']}, sweep=True)
    case('restartScript', ['p'], nodes=act, answers={'ScriptProcessor.getActiveScript': [None]})
    case('keepRunning', ['p'], nodes=act, world={'ticks': 30}, answers={'ScriptProcessor.getActiveScript': ['act']})
    case('keepRunning', ['p'], nodes=act, answers={'ScriptProcessor.getActiveScript': [None]})
    case('stopExecuting', ['p'], nodes=act, answers={'ScriptProcessor.getActiveScript': ['per']}, sweep=True)
    case('stopExecuting', ['p'], nodes=act, answers={'ScriptProcessor.getActiveScript': ['act']})
    case('stopExecuting', ['p'], nodes=act, answers={'ScriptProcessor.getActiveScript': ['per', None]})
    case('queueScript', ['p', 'q'], world={'ticks': 30}, steps=[{'step': 'script', 'index': 0, 'stage': 2}], answers={'@lambda': [True]}, sweep=True)
    case('queueScript(all)', ['n', 4, 'STRONG', True, 'q'], nodes=NPC, world={'ticks': 30})
    case('delayClock', ['p', 6, 4], nodes=clocks, world={'ticks': 30})
    case('delayClock(script)', ['p', 1, 4, True], nodes=dict(clocks, **act), world={'ticks': 30}, answers={'ScriptProcessor.getActiveScript': ['act']})
    case('delayClock(script)', ['p', 1, 4, False], nodes=clocks, world={'ticks': 30})
    case('delayClock', ['p', 9, 4], nodes=clocks)
    for t in (14, 15, 16):
        case('clockReady', ['p', 6], nodes=clocks, world={'ticks': t})
    case('delayAttack', ['p', 5], nodes=clocks, world={'ticks': 30}, sweep=True)
    case('stun', ['p', 4], nodes=clocks, world={'ticks': 30}, sweep=True)
    case('stun', ['n', 4], nodes=NPC, world={'ticks': 30})
    case('stun(message)', ['p', 4, False], world={'ticks': 30})
    case('setCurrentScriptState', ['p', 3], nodes=act, answers={'ScriptProcessor.getActiveScript': ['act']})
    for dev in (None, False, True):
        case('setCurrentScriptState', ['p', 3], answers={'ScriptProcessor.getActiveScript': [None]}, world={'devMode': dev}, sweep=dev is True)


def quests():
    case('getQuestPoints', ['p'], answers={'QuestRepository.getPoints': [12]})
    case('getQuestStage', ['p', 'COOKS_ASSISTANT'], answers={'QuestRepository.getStage': [20]})
    case('setQuestStage', ['p', 'COOKS_ASSISTANT', 30], answers={'QuestRepository.getQuests': [{'COOKS_ASSISTANT': 'cook'}]}, sweep=True)
    case('setQuestStage', ['p', 'COOKS_ASSISTANT', 30], answers={'QuestRepository.getQuests': [{'DEMON_SLAYER': 'demon'}]})
    for stage in (9, 10, 15, 20, 21):
        case('isQuestInProgress', ['p', 'COOKS_ASSISTANT', 10, 20], answers={'QuestRepository.getStage': [stage]})
    for stage in (0, 1, 100):
        case('isQuestStarted', ['p', 'DEMON_SLAYER'], answers={'QuestRepository.getStage': [stage]})
        case('isQuestComplete', ['p', 'DEMON_SLAYER'], answers={'QuestRepository.getStage': [stage]})
    case('getQuest', ['p', 'DEMON_SLAYER'], answers={'QuestRepository.getQuest': ['demon']})
    case('getQuest', ['p', 'DEMON_SLAYER'], answers={'QuestRepository.getQuest': [None]})
    case('startQuest', ['p', 'DEMON_SLAYER'], answers={'QuestRepository.getQuest': ['demon'], 'Quest.hasRequirements': [True]}, sweep=True)
    case('startQuest', ['p', 'DEMON_SLAYER'], answers={'QuestRepository.getQuest': ['demon'], 'Quest.hasRequirements': [False]})
    case('finishQuest', ['p', 'DEMON_SLAYER'], answers={'QuestRepository.getQuest': ['demon']})
    case('requireQuest', ['p', 'COOKS_ASSISTANT', 'to do this.'], answers={'QuestRepository.getStage': [100]})
    case('requireQuest', ['p', 'COOKS_ASSISTANT', 'to do this.'], answers={'QuestRepository.getStage': [50]}, sweep=True)
    ev = lambda met, unmet: {'Requirement.evaluate': [{'met': met, 'unmet': unmet}]}
    case('hasRequirement(req)', ['p', 'DRAGON_SLAYER'], answers=dict(ev(True, []), **{'QuestRepository.getAvailablePoints': [40], 'QuestRepository.getPoints': [3]}))
    case('hasRequirement(req)', ['p', 'DRAGON_SLAYER'], answers=dict(ev(True, [['QPReq', 32], ['QPCumulative', 5], ['QPCumulative', 2], ['QPReq', 10]]), **{'QuestRepository.getAvailablePoints': [40], 'QuestRepository.getPoints': [32]}), sweep=True)
    case('hasRequirement(req)', ['p', 'DRAGON_SLAYER'], answers=dict(ev(True, [['QPReq', 32], ['QPCumulative', 50]]), **{'QuestRepository.getAvailablePoints': [40], 'QuestRepository.getPoints': [39]}))
    case('hasRequirement(req)', ['p', 'DRAGON_SLAYER'], answers=dict(ev(False, [['QuestReq', 'COOK_ASSIST', 100], ['QPCumulative', 1]]), **{'QuestRepository.getAvailablePoints': [40], 'QuestRepository.getPoints': [39]}), sweep=True)
    case('hasRequirement(req)', ['p', 'DRAGON_SLAYER'], answers=dict(ev(False, [['QPCumulative', 1], ['QuestReq', 'COOK_ASSIST', 100]]), **{'QuestRepository.getAvailablePoints': [40], 'QuestRepository.getPoints': [39]}))
    case('hasRequirement(req)', ['p', 'DRAGON_SLAYER'], answers=dict(ev(False, [['SkillReq', 14, 10, True]]), **{'QuestRepository.getAvailablePoints': [0], 'QuestRepository.getPoints': [0]}))
    case('hasRequirement(req,message)', ['p', 'DRAGON_SLAYER', False], answers=dict(ev(False, []), **{'QuestRepository.getAvailablePoints': [0], 'QuestRepository.getPoints': [0]}))
    case('hasRequirement(quest)', ['p', 'DRAGON_SLAYER'], answers=dict(ev(False, []), **{'QuestRepository.getAvailablePoints': [0], 'QuestRepository.getPoints': [0]}), sweep=True)
    case('hasRequirement(quest)', ['p', 'MYTHS_OF_THE_WHITE_LANDS'])
    case('hasRequirement(quest,message)', ['p', 'COOKS_ASSISTANT', False], answers=dict(ev(True, []), **{'QuestRepository.getAvailablePoints': [0], 'QuestRepository.getPoints': [0]}))
    quests_kt = (ROOT / 'reference/server/Server/src/main/content/data/Quests.kt').read_text(encoding='utf-8')
    for q in re.findall(r'^\s+([A-Z_0-9]+)\("', quests_kt, re.M):
        case('hasRequirement(quest,message)', ['p', q, False], answers=dict(ev(True, []), **{'QuestRepository.getAvailablePoints': [0], 'QuestRepository.getPoints': [0]}), name=f'table:{q}')
    case('finishTask', ['p', 'LUMBRIDGE', 'BEGINNER', 3], sweep=True)
    case('finishTask', ['p', 'KARAMJA', 'BEGINNER', 3])
    for d in ('KARAMJA', 'VARROCK', 'LUMBRIDGE'):
        for lvl in ('BEGINNER', 'EASY', 'MEDIUM', 'HARD'):
            case('getDiaryLevelIndex', [d, lvl])


def content_services():
    case('getSlayerTask', ['p'], answers={'SlayerManager.getActiveTask': ['BANSHEE']})
    case('getSlayerTask', ['p'], answers={'SlayerManager.getActiveTask': [None]})
    case('getSlayerTaskName', ['p'], answers={'SlayerManager.getTaskName': ['banshees']})
    case('getSlayerTaskName', ['p'], answers={'SlayerManager.getTaskName': [None]})
    case('getSlayerTaskKillsRemaining', ['p'], answers={'SlayerManager.getAmount': [14]})
    mnpc = {'m': {'kind': 'npc', 'id': 8273}}
    case('getSlayerMaster', ['p'], nodes=mnpc, answers={'SlayerManager.getMaster': ['TURAEL'], 'Repository.findNPC': ['m']}, sweep=True)
    case('getSlayerMaster', ['p'], answers={'SlayerManager.getMaster': [None]})
    case('getSlayerMaster', ['p'], answers={'SlayerManager.getMaster': ['TURAEL'], 'Repository.findNPC': [None]})
    for master, nid in (('CHAELDAR', 1598), ('DURADEL', 8275), ('MAZCHNA', 8274), ('TURAEL', 8273), ('VANNAKA', 1597), ('VANNAKA', 1)):
        case('getSlayerMasterLocation', ['p'], nodes={'m': {'kind': 'npc', 'id': nid}}, answers={'SlayerManager.getMaster': [master], 'Repository.findNPC': ['m']})
    case('getSlayerTip', ['p'], answers={'SlayerManager.hasTask': [False]})
    case('getSlayerTip', ['p'], answers={'SlayerManager.hasTask': [True], 'SlayerManager.getActiveTask': ['BANSHEE'], 'Tasks.getTip': [['Bring earmuffs.']]}, sweep=True)
    case('getSlayerTip', ['p'], answers={'SlayerManager.hasTask': [True], 'SlayerManager.getActiveTask': [None]})
    case('getSlayerTip', ['p'], answers={'SlayerManager.hasTask': [True], 'SlayerManager.getActiveTask': ['BANSHEE'], 'Tasks.getTip': [None]})
    case('getSlayerTaskFlags', ['p'], answers={'SlayerFlags.getTaskFlags': [0x55]}, sweep=True)
    case('getSlayerFlags', ['p'])
    case('hasSlayerTask', ['p'], answers={'SlayerManager.hasTask': [True]})
    for iron, mode, want in ((False, 'ULTIMATE', 'STANDARD'), (True, 'STANDARD', 'STANDARD'), (True, 'STANDARD', 'ULTIMATE'), (True, 'HARDCORE', 'STANDARD'), (True, 'NONE', 'NONE')):
        case('hasIronmanRestriction', ['p', want], answers={'IronmanManager.isIronman': [iron], 'IronmanManager.getMode': [mode]})
    case('restrictForIronman', ['p', 'ULTIMATE', 't'], answers={'IronmanManager.checkRestriction': [False]}, sweep=True)
    case('restrictForIronman', ['p', 'ULTIMATE', 't'], answers={'IronmanManager.checkRestriction': [True]})
    for fn in ('openGrandExchange', 'openGrandExchangeCollectionBox', 'openBankAccount', 'openDepositBox', 'openBankPinSettings'):
        case(fn, ['p'], nodes={'p': dict(PLAYER['p'], records=[None] * 6)}, answers={'IronmanManager.checkRestriction': [False]}, sweep=True)
        case(fn, ['p'], nodes={'p': dict(PLAYER['p'], records=[None] * 6)}, answers={'IronmanManager.checkRestriction': [True]})
    ge = {'p': dict(PLAYER['p'], records=[None, 1, 1, None, None, None]), 'o': {'kind': 'offer', 'withdraw': ['w', None]}, 'o2': {'kind': 'offer'}, 'w': item(995, 5)}
    case('hasAwaitingGrandExchangeCollections', ['p'], nodes=ge, answers={'GrandExchangeRecords.getOffer': ['o']}, sweep=True)
    case('hasAwaitingGrandExchangeCollections', ['p'], nodes=ge, answers={'GrandExchangeRecords.getOffer': ['o2', 'o']})
    case('hasAwaitingGrandExchangeCollections', ['p'], nodes=ge, answers={'GrandExchangeRecords.getOffer': [None]})
    case('hasAwaitingGrandExchangeCollections', ['p'], nodes={'p': dict(PLAYER['p'], records=[])})
    bank = {'b2': item(995, 3000000), 'b3': item(995, 2500000)}
    sec = {'Entity.getAttribute': [None], 'IronmanManager.isIronman': [False]}
    case('activateSecondaryBankAccount', ['p'], answers={'IronmanManager.isIronman': [True], 'IronmanManager.getMode': ['ULTIMATE']})
    case('activateSecondaryBankAccount', ['p'], answers={'IronmanManager.isIronman': [False], 'Entity.getAttribute': [True]})
    case('activateSecondaryBankAccount', ['p'], nodes=bank, answers=dict(sec, **{'Container.getAmount': [100], 'Container.toArray': [['b2'], []]}))
    case('activateSecondaryBankAccount', ['p'], nodes=bank, answers=dict(sec, **{'Container.getAmount': [6000000], 'Container.toArray': [[], []], 'Container.remove': [True]}), sweep=True)
    case('activateSecondaryBankAccount', ['p'], nodes=bank, answers=dict(sec, **{'Container.getAmount': [2500000], 'Container.toArray': [['b2'], ['b3']], 'Container.remove': [True, False, True]}), sweep=True)
    case('activateSecondaryBankAccount', ['p'], nodes=bank, answers=dict(sec, **{'Container.getAmount': [2500000], 'Container.toArray': [['b2'], ['b3']], 'Container.remove': [False]}))
    case('activateSecondaryBankAccount', ['p'], nodes=bank, answers=dict(sec, **{'Container.getAmount': [5000000], 'Container.toArray': [[], []], 'Container.remove': [False]}))
    case('hasActivatedSecondaryBankAccount', ['p'], answers={'Entity.getAttribute': [True]})
    case('toggleBankAccount', ['p'], answers={'IronmanManager.checkRestriction': [False], 'Entity.getAttribute': [True]}, sweep=True)
    case('toggleBankAccount', ['p'], nodes={'p': dict(PLAYER['p'], secondary=True)}, answers={'IronmanManager.checkRestriction': [False], 'Entity.getAttribute': [True]})
    case('toggleBankAccount', ['p'], answers={'IronmanManager.checkRestriction': [False], 'Entity.getAttribute': [None]})
    case('toggleBankAccount', ['p'], answers={'IronmanManager.checkRestriction': [True]})
    for secondary in (False, True):
        sp = {'p': dict(PLAYER['p'], secondary=secondary)}
        case('isUsingSecondaryBankAccount', ['p'], nodes=sp)
        case('getBankAccountName', ['p'], nodes=sp)
        case('getBankAccountName(invert)', ['p', True], nodes=sp)
    case('openNpcShop', ['p', 520], nodes={'shop': {'kind': 'shop'}}, world={'shops': {'520': 'shop'}}, sweep=True)
    case('openNpcShop', ['p', 521], nodes={'shop': {'kind': 'shop'}}, world={'shops': {'520': 'shop'}})
    case('hasHouse', ['p'], answers={'HouseManager.hasHouse': [True]})
    store = lambda storable, read, setbit=(0, 8): {'StorageInterface.findStorableByDisplayId': [storable], 'BookcaseBitfields.get': [[0, 0]], 'BookcaseBitfields.isBitSet': [read], 'BookcaseBitfields.setBit': [list(setbit)]}
    case('storeBookInHouse(id)', ['p', 1511], answers=store(None, False))
    case('storeBookInHouse(id)', ['p', 1511], answers=store(-1, False))
    case('storeBookInHouse(id)', ['p', 1511], answers=store(3, False), sweep=True)
    case('storeBookInHouse(id)', ['p', 1511], answers=store(70, True))
    case('storeBookInHouse(id)', ['p', 7681], answers=store(9, False), sweep=True)
    case('storeBookInHouse(id)', ['p', 7681], answers=store(9, True), sweep=True)
    case('storeBookInHouse(node)', ['p', 'i1'], nodes={'i1': item(1511)}, answers=store(3, False))
    case('storeBookInHouse(node)', ['p', 'n'], nodes=NPC)
    case('getCutscene', ['p'], nodes={'cs': {'kind': 'cutscene'}}, answers={'Entity.getAttribute': [{'node': 'cs'}]})
    case('getCutscene', ['p'], answers={'Entity.getAttribute': [None]})
    case('getCutscene', ['p'], answers={'Entity.getAttribute': [4]})
    case('getCutsceneStage', ['p'], answers={'Entity.getAttribute': [None]})
    case('getCutsceneStage', ['p'], answers={'Entity.getAttribute': [6]})
    case('updateCredits', ['p', 5], answers={'UserAccountInfo.getCredits': [10]}, sweep=True)
    case('updateCredits', ['p', -11], answers={'UserAccountInfo.getCredits': [10]})
    case('updateCredits', ['p', -10], answers={'UserAccountInfo.getCredits': [10]})
    case('getCredits', ['p'], answers={'UserAccountInfo.getCredits': [4]})
    case('announceIfRare', ['p', 'r'], nodes={'r': item(1540, 2)}, sweep=True)
    case('announceIfRare', ['p', 'r'], nodes={'r': item(1511, 2)})


def logging():
    for level in ('FINE', 'INFO', 'WARN', 'ERR', 'DEBUG'):
        case('log', [level, f'{level} message'])
    case('logWithStack', ['ERR', 'Something broke'])
    case('exceptionToString', ['boom'])
    case('getServerConfig', [], nodes={'t': {'kind': 'toml'}}, world={'toml': 't'})
    case('getServerConfig', [])


def other_files():
    texts = ['Short.', ' '.join(['word'] * 20), '<col=ff0000>' + 'tagged words here ' * 5 + '</col>', 'a ' * 120, ' '.join(['abcdefghij'] * 30), '', 'x' * 60,
             '<b>bold</b> text <i>with</i> ' + 'many tags ' * 8, 'one two three four five six seven eight nine ten ' * 4]
    for t in texts:
        case('splitLines', [t], items=False)
        case('splitLines(limit)', [t, 20], items=False)
    for name in ('head', 'HELMET', 'Bracelet', 'ammunition', 'nothing', 'off', 'main', 'back', 'feet', 'ring'):
        case('EquipmentSlot.slotByName', [name], items=False)
    for god in god_items():
        case('God.validItems', [god], items=False)
    for steps in ([['enableOption', 0]], [['enableOption', 9], ['enableExamine']], [['enableOption', 10]], [['enableOption', -1]],
                  [['enableOptions(ids)', 1, 3, 5]], [['enableOptions(range)', 0, 9]], [['enableOptions(names)', 'a', None, 'c']],
                  [['enableAllOptions'], ['setUseOnSettings', True, False, True, False, True, True]], [['setInterfaceEventsDepth', 7], ['setInterfaceEventsDepth', 2]],
                  [['setInterfaceEventsDepth', 8]], [['enableSlotSwitch'], ['enableUseOption'], ['enableUseOnSelf'], ['enableNullSlotSwitch']], []):
        case('IfaceSettingsBuilder', [steps], items=False)
    case('asItem', [995])
    for a, b, s in ((0, 9, 1), (10, 0, -3), (1, 1, 1), (5, 4, 1)):
        case('IntProgression.toIntArray', [a, b, s], items=False)
    case('Collection<IntArray>.toIntArray', [[[1, 2], [], [3]]], items=False)
    for arr, el in (([1, 2, 3], 3), ([1, 2, 3], 2), ([1, 2, 3], 9), ([], 1), ([4, 4], 4)):
        for fn in ('IntArray.isLast', 'IntArray.getNext', 'IntArray.isNextLast', 'Collection.isLast', 'Collection.getNext', 'Collection.isNextLast'):
            case(fn, [arr, el], items=False)
    case('tryPop', [[1, 2], 9], items=False)
    case('tryPop', [[], 9], items=False)
    case('tryPop', [[], None], items=False)
    for name in ('ZAMORAK', 'zamorak', 'NONE'):
        case('parseEnumEntry<God>', [name], items=False)
    case('Event', [], items=False)
    case('Commands.reject', ['p', ['bad', 'worse']], sweep=True)
    case('Commands.reject', ['p', []])
    case('Commands.notify', ['p', 'done'])
    case('Commands.notify(log)', ['p', 'logged', True])
    case('Commands.notify(log)', ['p', 'quiet', False])
    case('Commands.define', ['cmd', 'h'])
    case('Commands.define(all)', ['cmd', 'MODERATOR', 'usage', 'desc', 'h'])
    case('MapArea.zone', [], items=False)
    for spec in ((3, 4, 1, -2, 1), (0, 0, 1, 1, 0), (-0.9, 0.1, 2, 2, 2), (0.1, -5, 0, 0, 3), (-3, -3, 1, 1, 0), (2.5, 3.7, -1, 0, 1)):
        case('Vector', [list(spec)], items=False)
    case('regionspec', ['empty'], answers={'DynamicRegion.reserveArea': [[64, 64, 127, 127, 0]]}, items=False)
    case('regionspec', ['copy'], nodes={'dyn': {'kind': 'region'}}, answers={'DynamicRegion.create': ['dyn']}, items=False)
    case('regionspec', ['using'], nodes={'dyn': {'kind': 'region'}}, answers={'DynamicRegion.create': ['dyn']}, items=False)
    fill = {'dyn': {'kind': 'region'}, 'src': {'kind': 'region'}, 'chunk': {'kind': 'chunk'}, 'copy': {'kind': 'chunk'}}
    case('regionspec', ['fill'], nodes=fill, answers={'@lambda': [False, True, False, False, True, False], 'RegionChunk.copy': ['copy', None]}, items=False, sweep=True)
    case('regionspec', ['fillDelegate'], nodes=fill, answers={'@lambda': [True, 'chunk', True, None, False], 'RegionChunk.copy': ['copy']}, items=False)
    case('regionspec', ['fillLateinit'], nodes=fill, items=False)


def cases():
    _cases.clear()
    FUNCTIONS.clear()
    for group in (skills, containers, visuals, scenery, world, interfaces, attributes_and_timers, varps, scripts, quests, content_services, logging, other_files):
        group()
    return list(_cases)
