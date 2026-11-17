"""Cases for the special attack oracle.

Each case is one world (entities, range tables, local entity lists, the draws)
and a list of operations; `cases()` answers (case, sweep) pairs where `sweep`
names the operation whose every recorded call and draw fails once in turn in a
second pass, or None. Handler indices: 0..2 are the CombatStyle singletons
(melee, range, magic), 3..36 the special classes in Support.SPECIALS order, 37
ChinchompaSwingHandler and 38 SalamanderSwingHandler.
"""
import copy
import struct

MARKERS = {'tick', 'update', 'runnerError'}
NAMES = [
    'AncientMace', 'Backstab', 'Chainhit', 'Cleave', 'Clobber', 'DescentOfDarkness', 'EnergyDrain', 'Excalibur', 'Feint',
    'Hamstring', 'HealingBlade', 'IceCleave', 'Impale', 'Judgement', 'Liquefy', 'PhantomStrike', 'Powershot', 'Powerstab',
    'Puncture', 'QuickSmash', 'Rampage', 'SaradominsLightning', 'Seercull', 'Sever', 'Shatter', 'Shove', 'SliceAndDice', 'Smash',
    'Snapshot', 'Snipe', 'SpearWall', 'Sweep', 'Warstrike', 'Weaken',
]
H = {name: 3 + k for k, name in enumerate(NAMES)}
WEAPON = {
    'AncientMace': 11061, 'Backstab': 8872, 'Chainhit': 805, 'Cleave': 1305, 'Clobber': 6739, 'DescentOfDarkness': 11235,
    'EnergyDrain': 4151, 'Excalibur': 35, 'Feint': 13899, 'Hamstring': 13883, 'HealingBlade': 11698, 'IceCleave': 11700,
    'Impale': 3101, 'Judgement': 11694, 'Liquefy': 11037, 'PhantomStrike': 13879, 'Powershot': 859, 'Powerstab': 7158,
    'Puncture': 1215, 'QuickSmash': 4153, 'Rampage': 1377, 'SaradominsLightning': 11730, 'Seercull': 6724, 'Sever': 4587,
    'Shatter': 1434, 'Shove': 1249, 'SliceAndDice': 14484, 'Smash': 13902, 'Snapshot': 861, 'Snipe': 8880, 'SpearWall': 13905,
    'Sweep': 3204, 'Warstrike': 11696, 'Weaken': 6746,
}
SWEPT = {'Backstab', 'DescentOfDarkness', 'Hamstring', 'Weaken'}
RANGED = {'Chainhit', 'DescentOfDarkness', 'Hamstring', 'PhantomStrike', 'Powershot', 'Seercull', 'Snapshot', 'Snipe'}
REGISTERED = {
    'melee': [11061, 8872, 8874, 8876, 8878, 1305, 13475, 6739, 4151, 35, 14632, 13899, 13901, 11698, 13452, 11700, 13453, 3101,
              11694, 13450, 11037, 7158, 1215, 1231, 5680, 5698, 13465, 13466, 13467, 13468, 4153, 1377, 11730, 4587, 13477, 1434,
              13479, 1249, 1263, 3176, 5716, 5730, 11716, 14662, 14484, 14486, 13902, 13904, 13905, 13907, 3204, 11696, 13451, 6746],
    'range': [805, 11235, 13405, 14803, 14804, 14805, 14806, 13883, 13879, 13880, 13881, 13882, 859, 10284, 6724, 861, 8880],
}
NEEDED_EVENTS = ['varp', 'message', 'registerTimer', 'spawnTimer', 'projectile', 'submit', 'chat', 'playAudio', 'clock',
                 'lockMovement', 'dumbPath', 'addPath', 'localEntities', 'localNpcs', 'localPlayers', 'findNpc', 'originalId',
                 'wildernessAttack', 'prayerToggle', 'continueAttack', 'rights', 'definitionName', 'setAttr', 'removeAttr',
                 'incPrayer', 'setPrayer', 'decPrayer', 'updateLevel', 'runEnergy', 'apiMessage', 'printStackTrace', 'log']


def hexd(value):
    return struct.pack('>d', value).hex()


ARROWS = 13
RANGE_TABLE = {
    'weapons': [
        {'itemId': 861, 'slot': ARROWS, 'type': 0, 'ammunition': [882, 892, 11212], 'animation': 426},
        {'itemId': 859, 'slot': ARROWS, 'type': 0, 'ammunition': [882, 892], 'animation': 426},
        {'itemId': 6724, 'slot': ARROWS, 'type': 0, 'ammunition': [882, 892], 'animation': 426},
        {'itemId': 11235, 'slot': ARROWS, 'type': 2, 'ammunition': [882, 892, 11212], 'animation': 426},
        {'itemId': 805, 'slot': 3, 'type': 3, 'ammunition': [805], 'animation': 929},
        {'itemId': 8880, 'slot': ARROWS, 'type': 1, 'ammunition': [8882], 'animation': 4230},
        {'itemId': 13883, 'slot': 3, 'type': 3, 'ammunition': [13883], 'animation': 10501},
        {'itemId': 13879, 'slot': 3, 'type': 3, 'ammunition': [13879], 'animation': 10501},
    ],
    'ammo': [
        {'itemId': 882, 'start': [19, 96, 0], 'darkBow': [1104, 96, 0], 'projectile': 10},
        {'itemId': 892, 'start': [21, 96, 0], 'darkBow': [1111, 96, 0], 'projectile': 15},
        {'itemId': 11212, 'start': [1116, 96, 0], 'darkBow': [1111, 96, 0], 'projectile': 1120},
        {'itemId': 805, 'start': [257, 96, 0], 'projectile': 258},
        {'itemId': 8882, 'start': [697, 96, 0], 'projectile': 698},
        {'itemId': 13883, 'start': [1838, 96, 0], 'projectile': 1839},
        {'itemId': 13879, 'start': [1836, 96, 0], 'projectile': 1837},
    ],
}
AMMO_FOR = {861: 892, 859: 892, 6724: 882, 11235: 892, 8880: 8882}
NAMES_OF = {882: 'Bronze arrow', 892: 'Rune arrow', 11212: 'Dragon arrow', 8882: 'Bone bolts'}


def levels(value=99):
    return {str(k): value for k in range(24)}


def player(weapon=None, **kw):
    spec = {'kind': 'player', 'specialToggled': True, 'specialEnergy': 100, 'levels': levels(), 'staticLevels': levels(),
            'bonuses': [90, 90, 90, 0, 90, 40, 40, 40, 0, 40, 0, 110, 0, 0, 80], 'lifepoints': 99, 'maxLifepoints': 99,
            'attackStyle': [0, 2], 'location': [3200, 3200, 0], 'prayerPoints': hexd(50.0)}
    if weapon is not None:
        equipment = {'3': [weapon, 1, 1000, 'weapon%d' % weapon]}
        if weapon in AMMO_FOR:
            ammo = AMMO_FOR[weapon]
            equipment[str(ARROWS)] = [ammo, 50, 1000, NAMES_OF[ammo]]
        if weapon in (805, 13883, 13879):
            equipment['3'] = [weapon, 20, 1000, 'weapon%d' % weapon]
        spec['equipment'] = equipment
    spec.update(kw)
    return spec


def npc(**kw):
    spec = {'kind': 'npc', 'id': 1, 'name': 'Guard', 'lifepoints': 99, 'maxLifepoints': 99, 'levels': levels(40),
            'staticLevels': levels(40), 'location': [3201, 3200, 0]}
    spec.update(kw)
    return spec


def victim_player(**kw):
    spec = player(None, location=[3201, 3200, 0], specialToggled=False)
    spec.update(kw)
    return spec


def world(entities, ops, seed=0, **kw):
    case = {'entities': entities, 'ops': ops, 'range': RANGE_TABLE, 'rng': {'mathSeed': seed, 'intSeed': seed * 7 + 1},
            'definitionNames': {str(k): v for k, v in NAMES_OF.items()}}
    case.update(kw)
    return case


def swing(name, ticks=4):
    return [{'op': 'swing', 'e': 0, 'v': 1, 'h': H[name]}, {'op': 'tick', 'n': ticks}]


def far(spec, distance=5):
    spec = dict(spec)
    spec['location'] = [3200 + distance, 3200, 0]
    return spec


def standard_cases():
    """Every special swung through CombatPulse.swing at an NPC and at a player, delivered by the runner."""
    out = []
    for name in NAMES:
        ranged = name in RANGED
        for seed in range(4):
            target = npc() if seed % 2 == 0 else victim_player()
            if ranged:
                target = far(target, 4 + seed)
            attacker = player(WEAPON[name])
            # The special bodies are swept in direct_cases; here a few whole swings with their deliveries.
            sweep = 0 if seed == 0 and name in SWEPT else None
            out.append((world([attacker, target], swing(name, 5), seed), sweep))
        # Too little energy, and the bar not toggled.
        low = player(WEAPON[name], specialEnergy=5)
        out.append((world([low, far(npc()) if ranged else npc()], swing(name, 1), 11), 0 if name in SWEPT else None))
        off = player(WEAPON[name], specialToggled=False)
        out.append((world([off, far(npc()) if ranged else npc()], swing(name, 1), 12), None))
    return out


def direct_cases():
    """Member calls with null arguments and edge states."""
    out = []
    for name in NAMES:
        h = H[name]
        attacker = player(WEAPON[name])
        target = far(npc()) if name in RANGED else npc()
        ops = [
            {'op': 'specialSwing', 'e': 0, 'v': 1, 'h': h, 'state': 'null'},
            {'op': 'set', 'e': 0, 'key': 'specialToggled', 'value': True},
            {'op': 'specialSwing', 'e': 0, 'h': h, 'state': {'attacker': 0}},
            {'op': 'set', 'e': 0, 'key': 'specialToggled', 'value': True},
            {'op': 'specialSwing', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
            {'op': 'visualize', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 5}},
            {'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 5}},
            {'op': 'impact', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 5, 'secondaryHit': 3}},
            {'op': 'adjust', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 7, 'secondaryHit': 2}},
            {'op': 'formattedHit', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}, 'hit': 150},
            {'op': 'tick', 'n': 3},
        ]
        out.append((world([attacker, target], ops, 3), 4))
    return out


def registry_cases():
    out = []
    ops = []
    for handler, ids in [(0, REGISTERED['melee']), (1, REGISTERED['range'])]:
        for item in ids + [1, 4153 if handler else 805, 10033]:
            ops.append({'op': 'getSpecial', 'h': handler, 'itemId': item})
    # A second newInstance finds its items registered: a log per item, and a chain stops at its first refusal.
    for name in ['Cleave', 'Backstab', 'DescentOfDarkness', 'Puncture', 'Snapshot']:
        ops.append({'op': 'newInstance', 'h': H[name]})
    out.append((world([player()], ops), len(ops) - 5))
    for osrs in [False, True]:
        ops = [{'op': 'fireEvent', 'h': H[name], 'e': 0, 'id': ident} for name in NAMES for ident in ['instant_spec', 'ncspec', 'x']]
        out.append((world([player()], ops, osrsDragonAxe=osrs), None))
    return out


def swing_handler_cases():
    out = []
    for weapon in [4151, 805, 861, 10033, 10034, 10146, 10149, 1, None]:
        for toggled in [True, False]:
            for style in ['MELEE', 'RANGE']:
                ops = [{'op': 'setStyle', 'e': 0, 'style': style},
                       {'op': 'getSwingHandler', 'e': 0, 'swing': True},
                       {'op': 'getSwingHandler', 'e': 0, 'swing': False}]
                sweep = 1 if weapon == 4151 and toggled and style == 'MELEE' else None
                out.append((world([player(weapon, specialToggled=toggled)], ops), sweep))
    spells = [{}]
    out.append((world([player(4151, spell=0)], [{'op': 'getSwingHandler', 'e': 0, 'swing': True}], spells=spells), None))
    out.append((world([player(4151, autocast=0)], [{'op': 'getSwingHandler', 'e': 0, 'swing': True},
                                                   {'op': 'getSwingHandler', 'e': 0, 'swing': False}], spells=spells), 0))
    return out


def instant_cases():
    out = []
    smash = H['QuickSmash']
    # The pulse victim adjacent, far, dead, refused by continueAttack, protected.
    for variant in range(7):
        target = npc()
        attacker = player(4153)
        if variant == 1:
            target['location'] = [3203, 3200, 0]
        if variant == 2:
            target['dead'] = True
        if variant == 3:
            attacker['continueAttack'] = False
        if variant == 4:
            target['protection'] = ['MELEE']
        if variant == 5:
            target = victim_player(protection=['MELEE'])
        if variant == 6:
            attacker['specialEnergy'] = 10
        ops = [{'op': 'setVictim', 'e': 0, 'v': 1}, {'op': 'instant', 'e': 0, 'h': smash}, {'op': 'tick', 'n': 2}]
        out.append((world([attacker, target], ops, variant), 1 if variant in (0, 5) else None))
    # No victim: the last victim within the window, outside it, out of reach, or none.
    for variant in range(5):
        attacker = player(4153, lastVictim=1, attrs={'combat-stop': 8})
        if variant == 1:
            attacker['attrs'] = {'combat-stop': 2}
        if variant == 2:
            del attacker['lastVictim']
        if variant == 3:
            attacker['location'] = [3210, 3200, 0]
        if variant == 4:
            attacker['attrs'] = {}
        ops = [{'op': 'instant', 'e': 0, 'h': smash}, {'op': 'tick', 'n': 2}]
        out.append((world([attacker, npc()], ops, 20 + variant, tick=10), 0 if variant == 0 else None))
    for weapon in [35, 14632, 1]:
        for nulls in [[], ['healovertime']]:
            ops = [{'op': 'instant', 'e': 0, 'h': H['Excalibur']}]
            out.append((world([player(weapon)], ops, nullTimers=nulls), 0 if not nulls else None))
    out.append((world([player(1377, levels={'0': 80, '1': 77, '2': 90, '4': 99, '6': 61})], [{'op': 'instant', 'e': 0, 'h': H['Rampage']}]), 0))
    for osrs in [False, True]:
        ops = [{'op': 'instant', 'e': 0, 'h': H['Clobber']}]
        out.append((world([player(6739), npc()], ops, osrsDragonAxe=osrs), 0 if osrs else None))
    return out


def multi_world(name, count, distance=1, attackable=None, seed=0, extra=None):
    """An attacker, its victim and `count` more NPCs around, all in a multi zone."""
    attacker = player(WEAPON[name], multiZone=True)
    victim = npc(multiZone=True)
    others = []
    for k in range(count):
        dx, dy = [(0, 1), (1, 1), (1, -1), (0, -1), (-1, 0), (-1, 1), (-1, -1), (2, 0), (2, 1), (2, -1), (0, 2), (0, -2), (1, 2), (1, -2),
                  (3, 0), (3, 1)][k % 16]
        spec = npc(id=10 + k, multiZone=k % 4 != 3, location=[3200 + dx, 3200 + dy, 0])
        if attackable is not None and k in attackable:
            spec['attackable'] = False
        others.append(spec)
    entities = [attacker, victim] + others
    tags = list(range(len(entities)))
    local = {}
    for key in ['localEntities|3200,3200,0|1', 'localEntities|3201,3200,0|1']:
        local[key] = tags
    local['localEntities|3201,3201,0|0'] = [t for t in [2, 0] if t < len(entities)]
    local['localEntities|3201,3199,0|0'] = [t for t in [4, 3] if t < len(entities)]
    if extra:
        local.update(extra)
    return entities, local


def multi_cases():
    out = []
    for name in ['Powerstab', 'SpearWall', 'Sweep']:
        for count in [0, 3, 16]:
            entities, local = multi_world(name, count, attackable={1, 5})
            ops = swing(name, 3)
            out.append((world(entities, ops, count, local=local), 0 if count == 3 and name == 'Sweep' else None))
        # Outside a multi zone the class falls back to its base swing.
        entities = [player(WEAPON[name]), npc()]
        out.append((world(entities, swing(name, 2), 5), None))
    # Sweep's large victim takes a second roll; the victim outside a multi zone keeps the state itself.
    entities, local = multi_world('Sweep', 4)
    entities[1]['size'] = 2
    entities[3]['size'] = 3
    out.append((world(entities, swing('Sweep', 2), 6, local=local), 0))
    entities, local = multi_world('Sweep', 2)
    entities[1]['multiZone'] = False
    out.append((world(entities, swing('Sweep', 2), 7, local=local), None))
    # The four target finders directly, at each direction from the attacker.
    for kind in ['targets', 'halberd', 'd2h', 'chinchompa']:
        for dx, dy in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (0, 0)]:
            entities, local = multi_world('Powerstab', 6, attackable={0, 2})
            entities[1]['location'] = [3200 + dx, 3200 + dy, 0]
            x, y = 3200 + dx, 3200 + dy
            sx, sy = (1 if dx > 0 else -1 if dx < 0 else 0), (1 if dy > 0 else -1 if dy < 0 else (0 if dx != 0 else 1))
            local['localEntities|%d,%d,0|1' % (x, y)] = list(range(len(entities)))
            local['localEntities|%d,%d,0|0' % (x - sy, y - sx)] = [0, 3, 4]
            local['localEntities|%d,%d,0|0' % (x + sy, y + sx)] = [5, 6]
            ops = [{'op': 'multihit', 'kind': kind, 'e': 0, 'v': 1, 'style': 'MELEE'}]
            out.append((world(entities, ops, local=local), 0 if (dx, dy) == (1, 0) else None))
    # Chinchompa targets around a player victim: players, familiars of local players, wilderness refusals.
    entities = [player(), victim_player(), victim_player(location=[3202, 3200, 0]),
                npc(kind='familiar', owner=2, location=[3201, 3201, 0]), npc(kind='familiar', owner=0, location=[3201, 3199, 0]),
                victim_player(location=[3202, 3201, 0], wildernessAttackable=False), npc(location=[3200, 3201, 0])]
    local = {'localEntities|3201,3200,0|1': list(range(len(entities)))}
    for target in [1, 6]:
        ops = [{'op': 'multihit', 'kind': 'chinchompa', 'e': 0, 'v': target}]
        out.append((world(entities, ops, local={**local, 'localEntities|3200,3201,0|1': list(range(len(entities)))}), 0))
    return out


def chainhit_cases():
    out = []
    h = H['Chainhit']
    # The barbarian assault door: supports missing, dead, transformed and present.
    door = npc(id=2440, location=[2544, 10143, 0])
    supports = [npc(id=2443, location=[2545, 10145, 0]), npc(id=2443, originalId=2444, location=[2545, 10141, 0]),
                npc(id=2443, dead=True, location=[2543, 10143, 0])]
    attacker = player(805, location=[2540, 10143, 0])
    npcs_at = {'2545,10145,0': 2, '2545,10141,0': 3, '2543,10143,0': 4}
    out.append((world([attacker, door] + supports, swing('Chainhit', 6), 1, npcsAt=npcs_at), 0))
    out.append((world([attacker, door] + supports, swing('Chainhit', 6), 2, npcsAt={'2545,10145,0': 2}), None))
    # Multi-zone bounces through local NPCs and players until the list or the energy runs out.
    for victim_kind in ['npc', 'player']:
        for energy in [100, 25, 15]:
            attacker = player(805, multiZone=True, specialEnergy=energy, location=[3200, 3200, 0])
            make = npc if victim_kind == 'npc' else victim_player
            victim = make(multiZone=True, location=[3204, 3200, 0])
            others = [make(multiZone=True, location=[3205, 3202, 0]), make(multiZone=False, location=[3203, 3203, 0]),
                      make(multiZone=True, location=[3206, 3198, 0], attackable=False), make(multiZone=True, location=[3209, 3200, 0])]
            entities = [attacker, victim] + others
            key = ('localNpcs' if victim_kind == 'npc' else 'localPlayers') + '|3200,3200,0|5'
            local = {key: [0, 2, 1, 3, 4, 5]}
            out.append((world(entities, swing('Chainhit', 12), energy, local=local), 1 if energy == 100 and victim_kind == 'npc' else None))
    # Already chaining.
    out.append((world([player(805, attrs={'chain-hit_v': []}), far(npc())], swing('Chainhit', 1)), 0))
    return out


def ranged_cases():
    out = []
    # Descent of darkness: dragon arrows, rune arrows, one arrow left, no arrows, no bow.
    for ammo, amount in [(11212, 50), (892, 50), (892, 1), (None, 0)]:
        attacker = player(11235)
        if ammo is None:
            del attacker['equipment'][str(ARROWS)]
        else:
            attacker['equipment'][str(ARROWS)] = [ammo, amount, 1000, NAMES_OF[ammo]]
        out.append((world([attacker, far(npc())], swing('DescentOfDarkness', 5), amount), 0 if amount == 1 else None))
    out.append((world([player(14806), far(npc())], swing('DescentOfDarkness', 1)), None))
    # Snapshot within two tiles and beyond; Snipe at a victim that is or is not fighting.
    for distance in [1, 2, 3, 7]:
        out.append((world([player(861), far(npc(), distance)], swing('Snapshot', 6), distance), 1 if distance == 7 else None))
    for attacking in [False, True]:
        target = far(npc(), 4)
        ops = ([{'op': 'attack', 'e': 1, 'v': 0}] if attacking else []) + swing('Snipe', 5)
        out.append((world([player(8880), target], ops, 3), None))
    # Seercull with the victim's magic drained or not.
    for drained in [False, True]:
        target = far(npc(levels={'6': 30} if drained else levels(40)))
        out.append((world([player(6724), target], swing('Seercull', 5), 4), None))
    # Hamstring and Phantom strike register timers; run them as the timer manager would.
    for name in ['Hamstring', 'PhantomStrike']:
        for kind in ['npc', 'player']:
            for seed in [0, 5, 9]:
                target = far(npc() if kind == 'npc' else victim_player(), 3)
                ops = swing(name, 4) + [{'op': 'timerRegister', 'timer': 0}] + [{'op': 'timerRun', 'timer': 0} for _ in range(5)]
                sweep = 3 if seed == 0 and kind == 'player' else None
                out.append((world([player(WEAPON[name]), target], ops, seed), sweep))
    return out


def melee_cases():
    out = []
    # Backstab and Snipe-like accuracy: the victim already fighting someone.
    for attacking in [False, True]:
        ops = ([{'op': 'attack', 'e': 1, 'v': 0}] if attacking else []) + swing('Backstab', 3)
        target = npc(staticLevels={'1': 40}, levels={'1': 30})
        out.append((world([player(8872), target], ops, 2), None))
    # Ancient mace with prayer below and above the static prayer level.
    for points in [10.0, 120.0]:
        out.append((world([player(11061, prayerPoints=hexd(points)), victim_player(prayerPoints=hexd(30.0))], swing('AncientMace', 3), 1), None))
    # Energy drain at a player; Sever at a player with protection prayers; Weaken's hit as the delay.
    out.append((world([player(4151), victim_player()], swing('EnergyDrain', 3), 2), None))
    prayers = ['PROTECT_FROM_MAGIC', 'PROTECT_FROM_MELEE', 'PROTECT_FROM_MISSILES', 'PROTECT_FROM_SUMMONING']
    for seed in range(3):
        out.append((world([player(4587), victim_player(prayers=prayers[seed:])], swing('Sever', 3), seed), 0 if seed == 0 else None))
    for script in [[hexd(0.99)], [hexd(0.0)]]:
        case = world([player(6746), victim_player()], swing('Weaken', 3), 0)
        case['rng'] = {'math': script, 'intSeed': 5}
        out.append((case, None))
    # Warstrike's drain chain: each skill answers what was left over.
    for returns in [[], [-5], [-5, -3], [-5, -3, 0, -2], [-5, -3, 0, -2, -1]]:
        target = victim_player(updateLevelReturns=returns, prayerPoints=hexd(1.0))
        case = world([player(11696), target], swing('Warstrike', 3), 0)
        case['rng'] = {'math': [hexd(0.0)], 'ints': ['max']}
        out.append((case, 0 if len(returns) == 5 else None))
    # Slice and dice: the first, second, third, fourth roll lands, or none does.
    for k in range(5):
        script = [hexd(0.99)] * k + [hexd(0.0)]
        case = world([player(14484), npc()], swing('SliceAndDice', 3), k)
        case['rng'] = {'math': script + [hexd(0.0)] * 4, 'intSeed': k}
        out.append((case, 0 if k == 4 else None))
    # Impale for an administrator; Healing blade's minimum restores; Ice cleave's freeze.
    out.append((world([player(3101, rights='ADMINISTRATOR'), npc()], swing('Impale', 2), 3), None))
    for script in [[hexd(0.0)], [hexd(0.99)]]:
        case = world([player(11698), npc()], swing('HealingBlade', 2), 0)
        case['rng'] = {'math': script, 'ints': [3]}
        out.append((case, None))
        case = world([player(11700), npc()], swing('IceCleave', 2), 0)
        case['rng'] = {'math': script, 'ints': [3]}
        out.append((case, None))
    case = world([player(11700), npc()], swing('IceCleave', 2), 0, nullTimers=['frozen'])
    case['rng'] = {'math': [hexd(0.0)], 'ints': [3]}
    out.append((case, None))
    # Shove in every direction, onto the same tile, at a large victim and into a wall.
    for dx, dy in [(0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (-1, 1), (1, -1), (-1, -1), (0, 0)]:
        target = npc(location=[3200 + dx, 3200 + dy, 0])
        out.append((world([player(1249), target], swing('Shove', 2), 1), 0 if (dx, dy) == (1, 1) else None))
    out.append((world([player(1249), npc(size=2)], swing('Shove', 1)), None))
    out.append((world([player(1249), npc(dumbPath=False)], swing('Shove', 1)), None))
    out.append((world([player(1249), victim_player()], swing('Shove', 1)), None))
    # Liquefy flips the bar through the setter, then swings as plain melee.
    out.append((world([player(11037), npc()], swing('Liquefy', 2), 3), 0))
    # Clobber: an NPC attacker answers -1 through Kotlin's safe cast, as does a null victim.
    out.append((world([npc(), npc()], [{'op': 'specialSwing', 'e': 0, 'v': 1, 'h': H['Clobber'], 'state': {'attacker': 0}}]), None))
    out.append((world([player(6739)], [{'op': 'specialSwing', 'e': 0, 'h': H['Clobber'], 'state': {'attacker': 0}}]), None))
    return out


def fight_cases():
    """Whole fights: CombatPulse asks Player.getSwingHandler, which answers the special while the bar is on."""
    out = []
    for name in ['Backstab', 'Cleave', 'Puncture', 'SaradominsLightning', 'Smash', 'Snapshot', 'Seercull', 'Powershot']:
        target = far(npc(), 4) if name in RANGED else npc()
        attacker = player(WEAPON[name], attackStyle=[0, 4] if name in RANGED else [0, 2])
        ops = [{'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 10}]
        out.append((world([attacker, target], ops, 6), None))
    return out


def cases():
    planned = []
    for group in [registry_cases, swing_handler_cases, standard_cases, direct_cases, instant_cases, multi_cases, chainhit_cases,
                  ranged_cases, melee_cases, fight_cases]:
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
