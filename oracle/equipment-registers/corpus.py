"""Cases for the equipment-registers oracle.

Each case is one world (entities, registers, range data, the draws) and a list
of operations; `cases()` answers (case, sweep) pairs where `sweep` names the
operation whose every recorded call and draw fails once in turn in a second
pass, or None. Handler indices: 0..2 are the CombatStyle singletons (melee,
range, magic), 3..5 the MixedSwingHandlers objects, 6
ChinchompaSwingHandler.getInstance() and 7 SalamanderSwingHandler.INSTANCE.
"""
import copy
import struct

MARKERS = {'tick', 'update', 'runnerError'}
CHIN, SAL = 6, 7
NEEDED_EVENTS = ['equip', 'equipRemove', 'equipAdd', 'equipRefresh', 'addItemOrDrop', 'sendMessage', 'onEquip', 'attr', 'sendDialogue',
                 'multiZone', 'localEntities', 'attackable', 'equipNew', 'nextInt', 'mathRandom', 'addXp', 'projectile', 'visualize',
                 'graphics', 'animate', 'message', 'apiChat', 'isDead', 'on', 'faceTemporary', 'questStarted', 'setAttr', 'apiAnimate',
                 'playAudio', 'invContains', 'hasOption', 'freeSlots', 'lock', 'walkReset', 'lockMovement', 'transform', 'apiAddItem',
                 'apiMessage', 'submit', 'reTransform', 'unlock', 'forceWalk', 'attrHas', 'sendGraphics', 'audio', 'hit', 'hurtAudio',
                 'removeAttr', 'sceneryHandler', 'sceneryOptionHandler', 'definePlugin', 'useWithHandler', 'dialogue', 'pulseRun',
                 'invRemove', 'invAdd', 'onUseWith', 'decantContainer', 'decantCalquat', 'apiRemoveItem', 'dialogueFile', 'dialogueNpc',
                 'dialoguePlayer', 'isKeg', 'combineKegs', 'pourIntoGlass', 'replaceSlot', 'printStackTrace', 'swingHandler']
BARROWS = [
    [4716, 4880, 4881, 4882, 4883, 4884], [4718, 4886, 4887, 4888, 4889, 4890], [4720, 4892, 4893, 4894, 4895, 4896],
    [4722, 4898, 4899, 4900, 4901, 4902], [4724, 4904, 4905, 4906, 4907, 4908], [4726, 4910, 4911, 4912, 4913, 4914],
    [4728, 4916, 4917, 4918, 4919, 4920], [4730, 4922, 4923, 4924, 4925, 4926], [4745, 4952, 4953, 4954, 4955, 4956],
    [4747, 4958, 4959, 4960, 4961, 4962], [4749, 4964, 4965, 4966, 4967, 4968], [4751, 4970, 4971, 4972, 4973, 4974],
    [4753, 4976, 4977, 4978, 4979, 4980], [4755, 4982, 4983, 4984, 4985, 4986], [4757, 4988, 4989, 4990, 4991, 4992],
    [4759, 4994, 4995, 4996, 4997, 4998], [4708, 4856, 4857, 4858, 4859, 4860], [4710, 4862, 4863, 4864, 4865, 4866],
    [4712, 4868, 4869, 4870, 4871, 4872], [4714, 4874, 4875, 4876, 4877, 4878], [4732, 4928, 4929, 4930, 4931, 4932],
    [4734, 4934, 4935, 4936, 4937, 4938], [4736, 4940, 4941, 4942, 4943, 4944], [4738, 4946, 4947, 4948, 4949, 4950],
]
TIER = 22500


def hexd(value):
    return struct.pack('>d', value).hex()


def levels(value=99):
    return {str(k): value for k in range(24)}


def player(**kw):
    spec = {'kind': 'player', 'levels': levels(70), 'staticLevels': levels(70), 'bonuses': [30, 30, 30, 10, 20, 40, 40, 40, 25, 40, 0, 50, 0, 7, 0],
            'lifepoints': 99, 'maxLifepoints': 99, 'attackStyle': [0, 2], 'location': [3200, 3200, 0], 'weaponInterface': 'UNARMED'}
    spec.update(kw)
    return spec


def npc(**kw):
    spec = {'kind': 'npc', 'id': 1, 'name': 'Man', 'lifepoints': 30, 'maxLifepoints': 30, 'levels': levels(20), 'staticLevels': levels(20),
            'location': [3201, 3200, 0], 'size': 1, 'bonuses': [0] * 15, 'combatAnims': [422, 404, 836], 'audio': [1, 2, 3]}
    spec.update(kw)
    return spec


def at(spec, dx, dy=0):
    spec = dict(spec)
    spec['location'] = [3200 + dx, 3200 + dy, 0]
    return spec


def world(entities, ops, seed=0, **kw):
    case = {'entities': entities, 'ops': ops, 'rng': {'mathSeed': seed, 'intSeed': seed * 7 + 1}}
    case.update(kw)
    return case


def scripted(case, ints=None, math=None):
    case['rng'] = {}
    if ints is not None:
        case['rng']['ints'] = ints
    if math is not None:
        case['rng']['math'] = [hexd(m) for m in math]
    return case


# ---------------------------------------------------------------- BarrowsEquipment
def barrows_cases():
    out = []
    for states in BARROWS:
        ops = []
        for index, item in enumerate(states):
            for charge in (0, 1, 1000, 11250, TIER - 1, TIER, TIER + 1, 30000, 65535):
                ops.append({'op': 'barrows', 'id': item, 'amount': 1 if charge != 1000 else 3, 'charge': charge})
        out.append((world([], ops), None))
    ops = [{'op': 'barrowsLists'}] + [{'op': 'barrows', 'id': i, 'charge': 5} for i in (0, 1, 4207, 4224, 13887, 4715, 4999, -1)]
    out.append((world([], ops), None))
    return out


# ---------------------------------------------------------------- registers and the degrader
def registry_cases():
    out = [(world([], [{'op': 'registry'}]), None),
           (world([], [{'op': 'registry'}, {'op': 'register', 'names': ['crystal', 'pvp']}, {'op': 'register', 'names': ['barrows']}],
                  registers=[]), None),
           (world([], [{'op': 'register', 'names': ['crystal', 'crystal']}], registers=['crystal']), None)]
    return out


def wearer(equipment, inventory=None, **kw):
    spec = player(**kw)
    spec['equipment'] = {str(slot): item for slot, item in equipment.items()}
    if inventory is not None:
        spec['inventory'] = {str(slot): item for slot, item in inventory.items()}
    return spec


def slots_for(*sets, slot=3):
    return {str(i): slot for s in sets for i in s}


def degrade_cases():
    out = []
    axe = BARROWS[1]
    names = ["Dharok's greataxe", "Dharok's axe 100", "Dharok's axe 75", "Dharok's axe 50", "Dharok's axe 25", "Dharok's axe 0"]
    # Every state of the weapon at the charges around each threshold, checked twice.
    for index, item in enumerate(axe):
        for charge in (0, 1, 2, 1000, TIER, TIER + 7, 65535):
            p = wearer({3: [item, 1, charge, names[index]]})
            ops = [{'op': 'weaponDegrades', 'e': 0}, {'op': 'weaponDegrades', 'e': 0}]
            out.append((world([p], ops, equipSlots=slots_for(axe)), None))
    # The first state with "100" in its name keeps its charge; an unregistered id is left alone.
    for name in ("Dharok's greataxe 100", 'Item 100'):
        out.append((world([wearer({3: [axe[0], 1, 5, name]})], [{'op': 'weaponDegrades', 'e': 0}], equipSlots=slots_for(axe)), None))
    out.append((world([wearer({3: [1277, 1, 1, 'Bronze sword']})], [{'op': 'weaponDegrades', 'e': 0}]), None))
    out.append((world([player()], [{'op': 'weaponDegrades', 'e': 0}, {'op': 'armourDegrades', 'e': 0}]), None))
    out.append((world([], [{'op': 'weaponDegrades', 'e': None}, {'op': 'armourDegrades', 'e': None}]), None))
    # A full Barrows set and other degradables in every armour slot, checked over several rounds.
    karil = {0: [4928, 1, 2, 'Karil\'s coif 100'], 1: [4224, 1, 1000, 'New crystal shield'], 2: [13887, 1, 1000, "Vesta's chainbody"],
             4: [4941, 1, 1, "Karil's top 75"], 5: [4233, 1, 1, 'Crystal shield 2/10'], 7: [4950, 1, 1, "Karil's skirt 0"],
             9: [13870, 1, 2, "Morrigan's leather body"], 10: [13889, 1, 1, "Corrupt Vesta's chainbody"], 12: [4746, 1, 1, 'Ring'],
             13: [4930, 1, 1, "Karil's coif 50"], 3: [4934, 1, 1, "Karil's x-bow 100"]}
    slots = {}
    for slot, item in karil.items():
        for s in BARROWS + [[13887, 13889], [13870, 13872], [4224, 4225, 4226, 4227, 4228, 4229, 4230, 4231, 4232, 4233, 4234, 4207]]:
            if item[0] in s:
                for i in s:
                    slots[str(i)] = slot
    ops = [{'op': 'armourDegrades', 'e': 0} for _ in range(3)] + [{'op': 'weaponDegrades', 'e': 0}]
    out.append((world([wearer(karil)], ops, equipSlots=slots), None))
    # Two-state PvP sets, crystal equipment to its seed, a refused add, other register orders.
    for item, name, charge in ((13887, "Vesta's chainbody", 7), (13889, "Corrupt Vesta's chainbody", 1), (13901, "Vesta's longsword (deg)", 1),
                               (4212, 'New crystal bow', 1000), (4223, 'Crystal bow 1/10', 1), (4207, 'Crystal seed', 1),
                               (4214, 'Crystal bow full', 250), (4214, 'Crystal bow full', 251)):
        p = wearer({3: [item, 1, charge, name]})
        sets = [[13899, 13901], [4212, 4214, 4215, 4216, 4217, 4218, 4219, 4220, 4221, 4222, 4223, 4207], [13887, 13889]]
        out.append((world([p], [{'op': 'weaponDegrades', 'e': 0}, {'op': 'weaponDegrades', 'e': 0}], equipSlots=slots_for(*sets)), None))
    out.append((world([wearer({3: [axe[1], 1, 1, "Dharok's axe 100"]})], [{'op': 'weaponDegrades', 'e': 0}]), None))
    for registers in ([], ['crystal'], ['crystal', 'pvp', 'barrows'], ['barrows', 'barrows']):
        p = wearer({3: [4207, 1, 1, 'Crystal seed'], 5: [axe[4], 1, 1, "Dharok's axe 25"], 4: [13889, 1, 1, 'Corrupt']})
        out.append((world([p], [{'op': 'weaponDegrades', 'e': 0}, {'op': 'armourDegrades', 'e': 0}], registers=registers,
                          equipSlots=slots_for(axe)), None))
    # Swept: the degrade to the next state, to broken (to the inventory or the ground) and to dust.
    out.append((world([wearer({3: [axe[2], 1, 1, "Dharok's axe 75"]})], [{'op': 'weaponDegrades', 'e': 0}], equipSlots=slots_for(axe)), 0))
    out.append((world([wearer({3: [axe[4], 1, 1, "Dharok's axe 25"]})], [{'op': 'weaponDegrades', 'e': 0}]), 0))
    out.append((world([wearer({4: [13889, 1, 1, 'Corrupt']})], [{'op': 'armourDegrades', 'e': 0}]), 0))
    out.append((world([wearer({3: [13899, 1, 3, "Vesta's longsword"]})], [{'op': 'weaponDegrades', 'e': 0}], equipSlots=slots_for([13899, 13901])), 0))
    # The join: CombatSwingHandler.adjustBattleState checks the player's weapon (melee and range singletons).
    for h, item, name in ((0, axe[1], "Dharok's axe 100"), (1, 4214, 'Crystal bow full'), (0, 1277, 'Bronze sword')):
        p = wearer({3: [item, 1, 1, name]})
        ops = [{'op': 'adjust', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 5, 'style': 'MELEE'}}]
        out.append((world([p, npc()], ops, equipSlots=slots_for(axe, [4214, 4215])), 0 if h == 0 and item == axe[1] else None))
    return out


# ---------------------------------------------------------------- HastaEquipListener
def hasta_cases():
    out = []
    definitions = [[11367, 'Bronze hasta'], [1, 'Toolkit'], [11369, 'Iron HASTA'], [13405, 'Hastaroth helm'], [4, 'Cannonball'],
                   [11419, 'Rune hasta(p++)'], [7, 'Has ta'], [11371, 'Steel hasta']]
    for value in (None, 0, 1, 2, 3, 7):
        p = player(attrs={} if value is None else {'/save:barbtraining:hasta': value}, inventory={'0': [11367, 1, 1000, 'Bronze hasta']})
        ops = [{'op': 'define', 'listener': 'hasta'}, {'op': 'equipHandler', 'e': 0, 'k': 0, 'node': {'slot': 0}}]
        out.append((world([p], ops, itemDefinitions=definitions), 1 if value == 0 else None))
    out.append((world([], [{'op': 'define', 'listener': 'hasta'}], itemDefinitions=[]), None))
    out.append((world([], [{'op': 'define', 'listener': 'hasta'}], itemDefinitions=[[1, 'Hasta'], [2, None], [3, 'hasta']]), None))
    out.append((world([], [{'op': 'define', 'listener': 'hasta'}], itemDefinitions=definitions), 0))
    return out


# ---------------------------------------------------------------- ChinchompaSwingHandler
CHIN_RANGE = {'weapons': [{'itemId': 10033, 'animation': 2779, 'attackSpeed': 4, 'slot': 3, 'type': 6, 'ammunition': [10033]},
                          {'itemId': 10034, 'animation': 2779, 'attackSpeed': 4, 'slot': 3, 'type': 6, 'ammunition': [10034]}],
              'ammo': [{'itemId': 10033, 'projectile': 908, 'start': [907, 96, 0]}, {'itemId': 10034, 'projectile': 909, 'start': None}]}


def thrower(item=10034, amount=50, **kw):
    return player(equipment={'3': [item, amount, 1000, 'Red chinchompa']}, attackStyle=[5, 4], weaponInterface='CHINCHOMPA', **kw)


def chinchompa_cases():
    out = []
    # The lookup Player.getSwingHandler makes, by weapon and pulse style.
    for item in (10033, 10034, 10146, 10149, 10150, 1277):
        for style in ('RANGE', 'MELEE'):
            p = player(equipment={'3': [item, 1, 1000, 'item']})
            ops = [{'op': 'setStyle', 'e': 0, 'style': style}, {'op': 'swingHandler', 'e': 0, 'swing': True},
                   {'op': 'swingHandler', 'e': 0, 'swing': False}]
            out.append((world([p], ops), None))
    # Outside multi combat: the range swing with the state as its own target, then every member.
    for distance in (1, 3, 7):
        for item in (10033, 10034):
            for hit in (0.0, 0.99):
                e = thrower(item)
                v = at(npc(), distance)
                state = {'attacker': 0, 'victim': 1}
                ops = [{'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': CHIN, 'state': state},
                       {'op': 'swing', 'e': 0, 'v': 1, 'h': CHIN}, {'op': 'tick', 'n': 4}]
                out.append((scripted(world([e, v], ops, range=CHIN_RANGE), math=[hit] * 4, ints=[3, 'max', 5, 2]), None))
    # Inside multi combat: up to nine more targets around the victim, by kind.
    ring = {'kind': 'npc'}
    for victim_kind in ('npc', 'player'):
        for accurate in (0.0, 0.99):
            entities = [thrower(multiZone=True), at(npc(multiZone=True) if victim_kind == 'npc' else player(multiZone=True), 3)]
            others = [at(npc(), 4), at(player(), 3, 1), at(npc(attackable=False), 2), at(npc(), 3, -1),
                      at(dict(npc(owner=0), kind='familiar'), 4, 1)]
            entities += others
            local = {'localEntities|3203,3200,0|1': [1, 0, 2, 3, 4, 5, 6]}
            ops = [{'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'attacker': 0, 'victim': 1}},
                   {'op': 'adjust', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'attacker': 0, 'victim': 1, 'targets': [
                       {'attacker': 0, 'victim': 1, 'estimatedHit': 4, 'style': 'RANGE'}, None, 'this',
                       {'attacker': 0, 'victim': 2, 'estimatedHit': 0, 'style': 'RANGE'}]}},
                   {'op': 'swing', 'e': 0, 'v': 1, 'h': CHIN}, {'op': 'tick', 'n': 5}]
            case = world(entities, ops, local=local, range=CHIN_RANGE)
            out.append((scripted(case, math=[accurate] * 6, ints=[1, 'max', 4, 0, 7, 3, 2, 9, 1, 1]),
                        0 if (victim_kind, accurate) == ('npc', 0.0) else None))
            if (victim_kind, accurate) == ('player', 0.99):
                # Swept: the whole CombatPulse.swing reaching every chinchompa member.
                swept = copy.deepcopy(case)
                swept['ops'] = [ops[2]]
                out.append((scripted(swept, math=[accurate] * 6, ints=[1, 'max', 4, 0, 7, 3, 2, 9, 1, 1]), 0))
    # A target list of ten or more, no range weapon, no ammunition, an NPC thrower.
    many = [thrower(multiZone=True), at(npc(multiZone=True), 2)] + [at(npc(), 2, k - 6) for k in range(12)]
    case = world(many, [{'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'attacker': 0, 'victim': 1}}],
                 local={'localEntities|3202,3200,0|1': list(range(14))}, range=CHIN_RANGE)
    out.append((scripted(case, math=[0.0], ints=[1] * 14), None))
    for e in (thrower(10035, multiZone=True), thrower(10034, 0, multiZone=True), thrower(10034, 1, multiZone=True)):
        case = world([e, at(npc(multiZone=True), 2)], [{'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'attacker': 0, 'victim': 1}}],
                     local={'localEntities|3202,3200,0|1': [1]}, range=CHIN_RANGE)
        out.append((scripted(case, math=[0.5], ints=[2]), None))
    # Members called directly.
    e = thrower()
    ops = []
    for style in ([5, 4], [6, 6], [4, 4]):
        ops.append({'op': 'set', 'e': 0, 'key': 'attackStyle', 'value': style})
        ops.append({'op': 'addExperience', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'targets': [
            {'estimatedHit': 7}, None, 'this', {'estimatedHit': -1}, {'estimatedHit': 3}], 'estimatedHit': 2}})
    ops += [{'op': 'addExperience', 'e': 0, 'v': 1, 'h': CHIN, 'state': {}},
            {'op': 'addExperience', 'e': 0, 'v': 1, 'h': CHIN, 'state': 'null'},
            {'op': 'adjust', 'e': 0, 'v': 1, 'h': CHIN, 'state': {}},
            {'op': 'adjust', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'targets': [{'attacker': 0, 'victim': None}]}},
            {'op': 'impact', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'targets': [None, {'attacker': 0, 'victim': 1, 'estimatedHit': 3}, 'this']}},
            {'op': 'impact', 'e': 0, 'v': 1, 'h': CHIN, 'state': {}},
            {'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'victim': 1, 'targets': [{'victim': 0}, None, 'this']}},
            {'op': 'visualizeImpact', 'e': 0, 'v': None, 'h': CHIN, 'state': {'targets': []}},
            {'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'targets': [{'victim': None}]}},
            {'op': 'visualize', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'weapon': 10034, 'ammunition': 10034}},
            {'op': 'visualize', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'weapon': 10033, 'ammunition': 10033}},
            {'op': 'visualize', 'e': 0, 'v': None, 'h': CHIN, 'state': {'weapon': 10033, 'ammunition': 10033}},
            {'op': 'visualize', 'e': 0, 'v': 1, 'h': CHIN, 'state': {'weapon': 1277}},
            {'op': 'set', 'e': 0, 'key': 'attackAnim', 'value': 2779},
            {'op': 'visualize', 'e': 0, 'v': 1, 'h': CHIN, 'state': {}},
            {'op': 'set', 'e': 0, 'key': 'attackAnim', 'value': 423},
            {'op': 'visualize', 'e': 0, 'v': 1, 'h': CHIN, 'state': {}},
            {'op': 'armourSet', 'e': 0, 'h': CHIN}, {'op': 'canSwing', 'e': 0, 'v': 1, 'h': CHIN}]
    out.append((world([e, at(npc(), 3)], ops, range=CHIN_RANGE), None))
    # Whole fights: the player's own lookup reaches the chinchompa handler every swing.
    for multi in (False, True):
        e = thrower(multiZone=multi)
        e['attackSpeed'] = 4
        entities = [e, at(npc(multiZone=multi, lifepoints=12), 4), at(npc(multiZone=multi), 5)]
        ops = [{'op': 'setStyle', 'e': 0, 'style': 'RANGE'}, {'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 12}]
        case = world(entities, ops, 3, local={'localEntities|3204,3200,0|1': [1, 2]}, range=CHIN_RANGE)
        out.append((case, None))
    return out


# ---------------------------------------------------------------- SalamanderSwingHandler
def salamander(item=10147, tar=10144, style=(4, 4), **kw):
    equipment = {'3': [item, 1, 1000, 'Red salamander']}
    if tar is not None:
        equipment['13'] = [tar, 30, 1000, 'Tarromin tar']
    return player(equipment=equipment, attackStyle=list(style), weaponInterface='SALAMANDER', **kw)


def salamander_cases():
    out = []
    styles = [(7, 7), (4, 4), (2, 1), (0, 0)]
    for style in styles:
        for item, tar in ((10147, 10144), (10149, 10142), (10146, 10143), (10148, 10145), (10148, 10142), (10147, None), (1277, 10144)):
            ops = [{'op': 'hit', 'e': 0, 'v': 1, 'h': SAL, 'modifier': hexd(1.0)},
                   {'op': 'hit', 'e': 0, 'v': 1, 'h': SAL, 'modifier': hexd(1.3)},
                   {'op': 'accuracy', 'e': 0, 'h': SAL}, {'op': 'defence', 'e': 1, 'v': 0, 'h': SAL},
                   {'op': 'setMultiplier', 'e': 0, 'h': SAL, 'skill': 6},
                   {'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': SAL, 'state': {'attacker': 0, 'victim': 1}},
                   {'op': 'visualize', 'e': 0, 'v': 1, 'h': SAL, 'state': {}},
                   {'op': 'impact', 'e': 0, 'v': 1, 'h': SAL, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 6, 'style': 'RANGE'}},
                   {'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': SAL, 'state': {'attacker': 0, 'victim': 1, 'spell': None}},
                   {'op': 'armourSet', 'e': 0, 'h': SAL}, {'op': 'postSwing', 'e': 0, 'v': 1, 'h': SAL, 'state': {}}]
            sweep = 5 if (style, item) == ((7, 7), 10147) else None
            out.append((scripted(world([salamander(item, tar, style), at(npc(), 1)], ops), math=[0.1, 0.99], ints=[4, 'max']), sweep))
    # The magic maximum of each salamander over levels where one bonus point crosses a boundary.
    for level in (1, 37, 50, 80, 99, 120):
        for item in (10146, 10147, 10148, 10149):
            e = salamander(item, style=(7, 7), levels=levels(level))
            ops = [{'op': 'hit', 'e': 0, 'v': 1, 'h': SAL, 'modifier': hexd(m)} for m in (1.0, 1.3, 2.5, 0.0)]
            out.append((world([e, at(npc(), 1)], ops), None))
    # canSwing at one to three tiles, still and moving victims.
    for distance in (0, 1, 2, 3):
        for moving in (False, True):
            for style in ((7, 7), (2, 1)):
                v = at(npc(moving=moving), distance)
                sweep = 0 if (distance, moving, style) == (2, True, (7, 7)) else None
                out.append((world([salamander(style=style), v], [{'op': 'canSwing', 'e': 0, 'v': 1, 'h': SAL}]), sweep))
    # Experience in each state style, a null attacker or state, a non-player, the lifepoints cap.
    ops = []
    for style in ('MAGIC', 'RANGE', 'MELEE', None):
        ops.append({'op': 'addExperience', 'e': 0, 'v': 1, 'h': SAL, 'state': {'estimatedHit': 9, 'style': style}})
    ops += [{'op': 'addExperience', 'e': None, 'v': 1, 'h': SAL, 'state': {}}, {'op': 'addExperience', 'e': 0, 'v': 1, 'h': SAL, 'state': 'null'},
            {'op': 'hit', 'e': None, 'v': 1, 'h': SAL, 'modifier': hexd(1.0)}, {'op': 'accuracy', 'e': None, 'h': SAL},
            {'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': SAL, 'state': 'null'},
            {'op': 'handlerSwing', 'e': 1, 'v': 0, 'h': SAL, 'state': {'attacker': 1, 'victim': 0}},
            {'op': 'set', 'e': 1, 'key': 'lifepoints', 'value': 2},
            {'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': SAL, 'state': {'attacker': 0, 'victim': 1}}]
    out.append((scripted(world([salamander(style=(7, 7), bonuses=[0] * 13 + [20, 0]), at(npc(), 1)], ops), math=[0.0] * 6, ints=['max'] * 6), None))
    out.append((world([salamander(style=(7, 7), bonuses=[0] * 5), at(npc(), 1)], [{'op': 'hit', 'e': 0, 'v': 1, 'h': SAL, 'modifier': hexd(1.0)}]), None))
    # The lifepoints cap at, just above and just below the hit.
    for draw in (1, 2, 3, 4):
        ops = [{'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': SAL, 'state': {'attacker': 0, 'victim': 1}}]
        out.append((scripted(world([salamander(style=(7, 7)), at(npc(lifepoints=2), 1)], ops), math=[0.0], ints=[draw]), None))
    # Whole fights: CombatPulse reaches the salamander through the player's lookup; range accurate attacks a tick sooner.
    for style in ((7, 7), (4, 4), (2, 1)):
        e = salamander(style=style)
        e['attackSpeed'] = 5
        ops = [{'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 14}]
        out.append((world([e, at(npc(lifepoints=40), 1)], ops, 5), None))
    return out


# ---------------------------------------------------------------- the creatures
def inventory(*items):
    return {str(k): item for k, item in enumerate(items)}


def sheep_cases():
    out = []
    for draws, kw in (([5], {}), ([4], {}), (['max'], {}), ([5], {'dead': True}), ([], {'attack': True})):
        s = npc(id=42, name='Sheep', dead=kw.get('dead', False))
        ops = [{'op': 'sheepTick', 'e': 1}]
        if kw.get('attack'):
            ops = [{'op': 'attack', 'e': 1, 'v': 0}, {'op': 'sheepTick', 'e': 1}]
        out.append((scripted(world([player(), s], ops), ints=draws), None))
    out.append((world([], [{'op': 'behaviourIds'}]), None))
    shears = [1735, 1, 1000, 'Shears']
    # Shearing: the penguin, no shears, a violent sheep, no room, the three in four, the escape.
    cases = [
        ({'id': 3579}, {'startedQuests': ['Sheep Shearer']}, [], None),
        ({'id': 3579}, {}, [], None),
        ({'id': 42}, {'inventory': inventory([995, 5, 1000, 'Coins'])}, [], None),
        ({'id': 43, 'options': ['attack', 'shear']}, {'inventory': inventory(shears)}, [], None),
        ({'id': 42}, {'inventory': {str(k): shears for k in range(28)}}, [], None),
        ({'id': 42}, {'inventory': inventory(shears)}, [0], 0),
        ({'id': 1765}, {'inventory': inventory(shears), 'addItem': [False]}, [2], None),
        ({'id': 42}, {'inventory': inventory(shears)}, [3], 0),
    ]
    for sheep_kw, player_kw, draws, sweep in cases:
        ops = [{'op': 'define', 'listener': 'sheep'}, {'op': 'npcOption', 'option': 'shear', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 81}]
        s = at(npc(name='Sheep', **sheep_kw), 1, 1)
        out.append((scripted(world([player(**player_kw), s], ops), ints=draws), None if sweep is None else 1))
    # The sheep backs away three "opposite" steps from every side (diagonals turn cardinal).
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            s = at(npc(id=42, name='Sheep'), 0, 0)
            p = player(location=[3200 + dx, 3200 + dy, 0], inventory=inventory(shears))
            ops = [{'op': 'define', 'listener': 'sheep'}, {'op': 'npcOption', 'option': 'shear', 'e': 0, 'v': 1}]
            out.append((scripted(world([p, s], ops), ints=[3]), None))
    return out


def chinchompa_death_cases():
    out = []
    for dx in (-2, -1, 0, 1, 2):
        for dy in (-1, 0, 1):
            k = player(location=[3200 + dx, 3200 + dy, 0])
            c = npc(id=5079, location=[3200, 3200, 0])
            out.append((scripted(world([c, k], [{'op': 'deathFinished', 'e': 0, 'v': 1}]), ints=[2]), None))
    for killer in (npc(id=5, location=[3201, 3200, 0]), player(location=[3201, 3200, 1]), player(location=[3201, 3201, 0], lifepoints=1)):
        out.append((scripted(world([npc(id=5080, location=[3200, 3200, 0]), killer], [{'op': 'deathFinished', 'e': 0, 'v': 1}]), ints=['max']), None))
    ops = [{'op': 'deathFinished', 'e': 0, 'v': 1}, {'op': 'respawn', 'e': 0}, {'op': 'respawn', 'e': 0},
           {'op': 'deathFinished', 'e': 0, 'v': 1}, {'op': 'deathFinished', 'e': 0, 'v': None}]
    case = world([npc(id=5079, attrs={'hunter': True}, location=[3200, 3200, 0]), player(location=[3199, 3199, 0])], ops)
    out.append((scripted(case, ints=[1, 0]), None))
    out.append((scripted(world([npc(id=5079, location=[3200, 3200, 0]), player(location=[3201, 3199, 0])],
                               [{'op': 'deathFinished', 'e': 0, 'v': 1}]), ints=[1]), 0))
    return out


def dog_cases():
    out = []
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            d = npc(id=4766, name='Stray dog', location=[3200, 3200, 0])
            p = player(location=[3200 + dx, 3200 + dy, 0])
            ops = [{'op': 'define', 'listener': 'dog'}, {'op': 'npcOption', 'option': 'shoo-away', 'e': 0, 'v': 1}]
            out.append((world([p, d], ops), 1 if (dx, dy) == (1, 0) else None))
    return out


def cow_cases():
    out = []
    out.append((world([], [{'op': 'cowPlugin'}]), 0))
    bucket, pail, milk = [1925, 1, 1000, 'Bucket'], [3727, 1, 1000, 'Pail'], [1927, 1, 1000, 'Bucket of milk']
    for items, option in (([], 'milk'), ([bucket], 'steal-cowbell'), ([bucket], 'STEAL-Cowbell'), ([bucket], 'milk'),
                          ([bucket, pail, bucket], 'milk'), ([pail], 'Milk'), ([milk, milk], 'milk')):
        p = player(inventory=inventory(*items))
        ops = [{'op': 'cowHandle', 'e': 0, 'node': {'scenery': 8689}, 'option': option}, {'op': 'tick', 'n': 30}]
        out.append((world([p], ops), None))
    p = player(inventory=inventory(bucket, bucket))
    out.append((world([p], [{'op': 'cowHandle', 'e': 0, 'node': None, 'option': 'milk'}, {'op': 'cowHandle', 'e': 0, 'node': None, 'option': None},
                            {'op': 'bucket', 'e': 0, 'used': {'slot': 0}, 'with': {'scenery': 12111}}, {'op': 'tick', 'n': 18}]), None))
    p = player(inventory=inventory(bucket))
    out.append((world([p], [{'op': 'bucket', 'e': 0, 'used': {'slot': 0}, 'with': {'scenery': 8689}}, {'op': 'set', 'e': 0, 'key': 'active', 'value': False},
                            {'op': 'tick', 'n': 9}]), None))
    p = player(inventory=inventory(bucket, pail))
    out.append((world([p], [{'op': 'cowHandle', 'e': 0, 'node': {'scenery': 8689}, 'option': 'milk'}, {'op': 'tick', 'n': 8}]), 1))
    return out


ATTACK = [2428, 121, 123, 125]
STRENGTH = [113, 115, 117, 119]


def potion(item, slot_name=None):
    return [item, 1, 1000, slot_name or 'potion%d' % item]


def decant_cases():
    out = []
    out.append((world([], [{'op': 'define', 'listener': 'decant'}]), 0))
    # The NPC option: potions and kegs decanted, a refused removal, the dialogue.
    for spec in ({'decant': [[[121, 1, 1000], [125, 1, 1000]], [[2428, 1, 1000]]], 'calquat': [[[5777, 1, 1000]], [[5779, 2, 1000]]]},
                 {'decant': [[[121, 1, 1000], [125, 1, 1000]], [[2428, 1, 1000]]], 'removeItem': [True, False]},
                 {'calquat': [[[5777, 1, 1000]], [[5779, 1, 1000]]], 'removeItem': [False]}, {}):
        ops = [{'op': 'define', 'listener': 'decant'}, {'op': 'npcOption', 'option': 'decant', 'e': 0, 'v': 1},
               {'op': 'decantDialogue', 'e': 0, 'n': 3}]
        out.append((world([player(**spec), npc(id=1990, name='Bob Barter')], ops), 1 if 'calquat' in spec and 'decant' in spec else None))
    # Combining doses by hand.
    pairs = [(ATTACK[3], ATTACK[3]), (ATTACK[3], ATTACK[2]), (ATTACK[2], ATTACK[2]), (ATTACK[1], ATTACK[1]), (ATTACK[3], ATTACK[1]),
             (ATTACK[2], ATTACK[1]), (ATTACK[0], ATTACK[3]), (ATTACK[2], ATTACK[0]), (ATTACK[2], STRENGTH[2]), (ATTACK[2], 229),
             (1, 2), (5777, ATTACK[2]), (ATTACK[3], 5779), (ATTACK[1], ATTACK[2])]
    for used, with_ in pairs:
        p = player(inventory={'3': potion(used), '7': potion(with_)})
        ops = [{'op': 'define', 'listener': 'decant'}, {'op': 'useWith', 'k': 0, 'e': 0, 'used': {'slot': 3}, 'with': {'slot': 7}}]
        out.append((world([p], ops), 1 if (used, with_) == (ATTACK[2], ATTACK[1]) else None))
    p = player(inventory={'3': potion(ATTACK[2])})
    out.append((world([p, npc()], [{'op': 'define', 'listener': 'decant'},
                                   {'op': 'useWith', 'k': 0, 'e': 0, 'used': {'slot': 3}, 'with': {'entity': 1}},
                                   {'op': 'useWith', 'k': 0, 'e': 0, 'used': {'entity': 1}, 'with': {'slot': 3}}]), None))
    # A keg poured into a beer glass, either way round.
    for used, with_ in ((1919, 5779), (5777, 1919), (5777, 5779)):
        p = player(inventory={'0': potion(used), '1': potion(with_)}, pourIntoGlass=[False])
        ops = [{'op': 'define', 'listener': 'decant'}, {'op': 'useWith', 'k': 1, 'e': 0, 'used': {'slot': 0}, 'with': {'slot': 1}}]
        out.append((world([p], ops), None))
    # Barbarian decanting.
    for herblore, used, with_ in ((0, ATTACK[0], 229), (1, ATTACK[0], 229), (2, 229, STRENGTH[0]), (1, ATTACK[1], 229), (1, 229, 1),
                                  (None, ATTACK[0], 229)):
        attrs = {} if herblore is None else {'/save:barbtraining:herblore': herblore}
        p = player(attrs=attrs, inventory={'4': potion(used), '9': potion(with_)})
        ops = [{'op': 'define', 'listener': 'decant'}, {'op': 'useWith', 'k': 2, 'e': 0, 'used': {'slot': 4}, 'with': {'slot': 9}}]
        out.append((world([p], ops), 1 if (herblore, used) == (1, ATTACK[0]) else None))
    return out


def cases():
    planned = []
    for group in [barrows_cases, registry_cases, degrade_cases, hasta_cases, chinchompa_cases, salamander_cases, sheep_cases,
                  chinchompa_death_cases, dog_cases, cow_cases, decant_cases]:
        for case, sweep in group():
            case = copy.deepcopy(case)
            case['group'] = group.__name__
            planned.append((case, sweep))
    return planned


def groups(cases):
    counts = {}
    for case in cases:
        counts[case['group']] = counts.get(case['group'], 0) + 1
    return dict(sorted(counts.items()))
