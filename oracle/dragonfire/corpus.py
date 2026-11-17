"""Cases for the dragonfire oracle.

Each case is one world (entities, handlers, the draws) and a list of
operations; `cases()` answers (case, sweep) pairs where `sweep` names the
operation whose every recorded call and draw fails once in turn in a second
pass, or None. Handler indices: 0..2 are the CombatStyle singletons (melee,
range, magic), 3..5 the MixedSwingHandlers objects (MagicalRanged,
MagicalMelee, RangedMelee), 6 ChinchompaSwingHandler, 7
SalamanderSwingHandler, then the case's own handlers in order.
"""
import copy
import struct

MARKERS = {'tick', 'update', 'runnerError'}
FIRST = 8
NEEDED_EVENTS = ['timerActive', 'apiMessage', 'message', 'spawnTimer', 'registerTimer', 'projectile', 'faceLocation',
                 'updateBonuses', 'playAudio', 'setVarp', 'isLocked', 'lockKey', 'delayEntity', 'replaceSlot', 'drop',
                 'itemPlugin', 'on', 'dragonfireProtection', 'fireResistance', 'setAttr', 'removeAttr', 'updateLevel', 'poison',
                 'graphics', 'visualize', 'equipReplace', 'audio', 'mathRandom', 'nextInt', 'submit', 'invGet', 'prayer',
                 'printStackTrace']
SHIELDS = [None, 1540, 11283, 11284, 2890, 9731, 1201]


def hexd(value):
    return struct.pack('>d', value).hex()


def levels(value=99):
    return {str(k): value for k in range(24)}


def player(shield=None, charge=1000, **kw):
    spec = {'kind': 'player', 'specialToggled': False, 'specialEnergy': 100, 'levels': levels(70), 'staticLevels': levels(70),
            'bonuses': [30, 30, 30, 10, 20, 40, 40, 40, 25, 40, 0, 50, 0, 0, 0], 'lifepoints': 99, 'maxLifepoints': 99,
            'attackStyle': [0, 2], 'location': [3200, 3200, 0], 'prayerPoints': hexd(50.0), 'weaponInterface': 'UNARMED'}
    if shield is not None:
        spec['equipment'] = {'5': [shield, 1, charge, 'shield%d' % shield]}
    spec.update(kw)
    return spec


def dragon(**kw):
    spec = {'kind': 'npc', 'id': 53, 'name': 'Red dragon', 'lifepoints': 140, 'maxLifepoints': 140, 'levels': levels(130),
            'staticLevels': levels(130), 'location': [3201, 3200, 0], 'size': 1, 'bonuses': [0] * 15, 'combatAnims': [80, 81, 81],
            'audio': [408, 410, 409]}
    spec.update(kw)
    return spec


def world(entities, ops, seed=0, handlers=None, **kw):
    case = {'entities': entities, 'ops': ops, 'rng': {'mathSeed': seed, 'intSeed': seed * 7 + 1}, 'handlers': handlers or []}
    case.update(kw)
    return case


def at(spec, dx, dy=0):
    spec = dict(spec)
    spec['location'] = [3200 + dx, 3200 + dy, 0]
    return spec


TEMPLATE = [None, None, 54, 40, 36, 41, 46, 20, 255]
MAGIC_TEMPLATE = [None, None, 500, 20, 20, 41, 40, 18, 255]


def get(melee=False, max_hit=52, fire=True, via='get', animation=81, start=None, end=None, projectile=None):
    return {'kind': 'dragonfire', 'via': via, 'meleeRange': melee, 'maximumHit': max_hit, 'fire': fire, 'animation': animation,
            'start': start, 'end': end, 'projectile': projectile}


def chromatic():
    """ChromaticDragonNPC's handler: two melee attacks and the melee-range dragonfire, no melee distance."""
    return [get(True, 52, True, start=[1, 64, 0]),
            {'kind': 'multi', 'meleeDistance': False, 'attacks': [
                {'handler': 0, 'animation': 80}, {'handler': 0, 'animation': 91}, {'from': FIRST}]}]


def metal():
    """MetalDragonNPC's handler: two melee attacks and the dragonfire projectile, melee distance."""
    return [get(False, 52, True, projectile=TEMPLATE),
            {'kind': 'multi', 'meleeDistance': True, 'attacks': [
                {'handler': 0, 'animation': 80}, {'handler': 0, 'animation': 91}, {'from': FIRST}]}]


def protection_cases():
    """calculateDragonfireMaxHit, hasDragonfireShieldProtection and getDragonfireProtection over every combination."""
    out = []
    for shield in SHIELDS:
        ops = []
        for potion in (False, True):
            for prayer in (False, True):
                ops.append({'op': 'set', 'e': 0, 'key': 'timers', 'value': ['DragonFireImmunity'] if potion else None})
                ops.append({'op': 'set', 'e': 0, 'key': 'prayers', 'value': ['PROTECT_FROM_MAGIC'] if prayer else None})
                for wyvern in (False, True):
                    for message in (False, True):
                        ops.append({'op': 'maxHit', 'e': 0, 'max': 52, 'wyvern': wyvern, 'unprotectable': 0, 'message': message})
                    ops.append({'op': 'maxHit', 'e': 0, 'max': 60, 'wyvern': wyvern, 'unprotectable': 10, 'message': True})
                    ops.append({'op': 'shieldProtection', 'e': 0, 'wyvern': wyvern})
                for fire in (False, True):
                    for resistant in (False, True):
                        ops.append({'op': 'set', 'e': 0, 'key': 'fireResistance', 'value': resistant})
                        ops.append({'op': 'protection', 'e': 0, 'fire': fire})
        out.append((world([player(shield)], ops), None))
    # Swept: every call of the full-message protection check fails once in turn.
    out.append((world([player(11283, timers=['DragonFireImmunity'], prayers=['PROTECT_FROM_MAGIC'])],
                      [{'op': 'maxHit', 'e': 0, 'max': 52, 'wyvern': False, 'unprotectable': 0, 'message': True}]), 0))
    out.append((world([player(2890, fireResistance=True, prayers=['PROTECT_FROM_MAGIC'])],
                      [{'op': 'protection', 'e': 0, 'fire': True}]), 0))
    # A non-player's protection tokens.
    ops = []
    for tokens in range(16):
        ops.append({'op': 'set', 'e': 0, 'key': 'dragonfireProtection', 'value': tokens})
        for wyvern in (False, True):
            ops.append({'op': 'maxHit', 'e': 0, 'max': 52, 'wyvern': wyvern, 'unprotectable': 0, 'message': True})
        ops.append({'op': 'maxHit', 'e': 0, 'max': 7, 'wyvern': False, 'unprotectable': 3, 'message': False})
        ops.append({'op': 'protection', 'e': 0, 'fire': tokens % 2 == 0})
    out.append((world([dragon()], ops), None))
    # Large and odd maximums, a negative minimum.
    ops = [{'op': 'maxHit', 'e': 0, 'max': m, 'wyvern': w, 'unprotectable': u, 'message': False}
           for m in (0, 1, 9, 11, 2147483647, -20) for w in (False, True) for u in (-5, 0)]
    out.append((world([player(1540, prayers=['PROTECT_FROM_MAGIC'])], ops), None))
    return out


def dragonfire_direct_cases():
    """Every DragonfireSwingHandler member called directly, at players and NPCs."""
    out = []
    handlers = [get(False, 52, True, projectile=TEMPLATE, end=[440, 0, 0]),   # 8: fiery breath from afar
                get(True, 52, True, start=[1, 64, 0]),                         # 9: fiery breath in melee range
                get(False, 54, False, via='get', animation=2988, start=[501, 0, 0]),  # 10: icy (wyvern) breath
                get(False, 50, True, via='get6', projectile=TEMPLATE),         # 11: the six-argument form
                {'kind': 'dragonfire', 'via': 'new', 'meleeRange': False, 'maximumHit': 56, 'fire': True, 'attack': None}]  # 12: KBD's
    for h in range(FIRST, FIRST + 5):
        for shield in (None, 11283, 1540, 2890):
            for distance in (1, 2, 5, 8):
                victim = at(player(shield), distance)
                ops = [{'op': 'canSwing', 'e': 0, 'v': 1, 'h': h},
                       {'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
                       {'op': 'visualize', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
                       {'op': 'adjust', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 4 + 7 * distance, 'style': 'MAGIC'}},
                       {'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 12}},
                       {'op': 'impact', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 12, 'secondaryHit': 3}}]
                sweep = 1 if (shield, distance) == (None, 5) else None
                out.append((world([dragon(), victim], ops, h * 10 + distance, handlers), sweep))
    # The members that do not reach a victim's state, the overridden calculations and the inherited ones.
    for h in range(FIRST, FIRST + 5):
        ops = [{'op': 'accuracy', 'e': 0, 'h': h}, {'op': 'hit', 'e': 0, 'v': 1, 'h': h, 'modifier': hexd(1.0)},
               {'op': 'defence', 'e': 1, 'v': 0, 'h': h}, {'op': 'setMultiplier', 'e': 0, 'h': h, 'skill': 6},
               {'op': 'formattedHit', 'e': 0, 'v': 1, 'h': h, 'hit': 150, 'state': {'attacker': 0, 'victim': 1}},
               {'op': 'formattedHit', 'e': 0, 'v': 1, 'h': h, 'hit': -1, 'state': {'attacker': 0, 'victim': 1}},
               {'op': 'armourSet', 'e': 0, 'h': h},
               {'op': 'addExperience', 'e': 1, 'v': 0, 'h': h, 'state': {'attacker': 1, 'victim': 0, 'estimatedHit': 9}},
               {'op': 'addExperience', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 9}},
               {'op': 'postSwing', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
               {'op': 'hit', 'e': 0, 'h': h, 'modifier': hexd(1.0)},
               {'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': h, 'state': 'null'},
               {'op': 'handlerSwing', 'e': 0, 'h': h, 'state': {'attacker': 0}},
               {'op': 'visualizeImpact', 'e': 0, 'h': h, 'state': {'attacker': 0}}]
        out.append((world([dragon(), player(11283, prayers=['PROTECT_FROM_MAGIC'])], ops, h, handlers), None))
    # A player's breath (the shield's own attack) and an NPC's at an NPC victim: no charging.
    for h in (FIRST, FIRST + 2):
        ops = [{'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
               {'op': 'visualizeImpact', 'e': 1, 'v': 0, 'h': h, 'state': {'attacker': 1, 'victim': 0}},
               {'op': 'adjust', 'e': 1, 'v': 0, 'h': h, 'state': {'attacker': 1, 'victim': 0, 'estimatedHit': 30}}]
        out.append((world([player(11283), dragon(dragonfireProtection=6)], ops, 3, handlers), None))
    # Charging the dragonfire shield: uncharged, empty, nearly full, full and above full.
    for shield, charge in [(11284, 1000), (11283, 0), (11283, 980), (11283, 990), (11283, 1000), (11283, 1500), (11283, 65535)]:
        ops = [{'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': FIRST, 'state': {'attacker': 0, 'victim': 1}}] * 3
        out.append((world([dragon(), player(shield, charge)], ops, 1, handlers), 0 if charge == 0 else None))
    # Icy breath freezes on a draw of 2 without frozen immunity; a null timer is not registered.
    for immune in (False, True):
        for draw in (2, 3):
            for nulls in ([], ['frozen']):
                case = world([dragon(), player(None, timers=['frozen:immunity'] if immune else [])],
                             [{'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': FIRST + 2, 'state': {'attacker': 0, 'victim': 1}}],
                             0, handlers, nullTimers=nulls)
                case['rng'] = {'math': [], 'ints': [draw]}
                out.append((case, 0 if (immune, draw, nulls) == (False, 2, []) else None))
    # Icy breath above 10 against the elemental and mind shields is redrawn.
    for shield in (2890, 9731, 1540, None):
        for hit in (5, 10, 11, 40):
            ops = [{'op': 'adjust', 'e': 0, 'v': 1, 'h': FIRST + 2, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': hit, 'secondaryHit': hit, 'style': 'RANGE'}}]
            out.append((world([dragon(), player(shield)], ops, hit, handlers), 0 if (shield, hit) == (2890, 40) else None))
    return out


def multi_cases():
    """MultiSwingHandler: switching, its delegations and the chromatic and metal dragons' tables."""
    out = []
    for table, name in [(chromatic(), 'chromatic'), (metal(), 'metal')]:
        h = FIRST + 1
        for distance in (1, 2, 4, 7):
            for seed in range(3):
                ops = []
                for _ in range(4):
                    ops += [{'op': 'canSwing', 'e': 0, 'v': 1, 'h': h},
                            {'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
                            {'op': 'visualize', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
                            {'op': 'adjust', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 14, 'style': 'MELEE'}},
                            {'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 14}},
                            {'op': 'impact', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 14, 'secondaryHit': 2}},
                            {'op': 'addExperience', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
                            {'op': 'postSwing', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}},
                            {'op': 'accuracy', 'e': 0, 'h': h}, {'op': 'defence', 'e': 1, 'v': 0, 'h': h},
                            {'op': 'hit', 'e': 0, 'v': 1, 'h': h, 'modifier': hexd(1.0)},
                            {'op': 'setMultiplier', 'e': 0, 'h': h, 'skill': 2}, {'op': 'armourSet', 'e': 0, 'h': h}]
                sweep = 1 if (distance, seed) == (2, 0) else None
                out.append((world([dragon(), at(player(11283 if seed == 1 else None), distance)], ops, seed + 3 * distance, table), sweep))
        # Scripted draws: every next index, at melee range and out of it.
        for distance in (1, 3):
            for first in range(3):
                case = world([dragon(), at(player(), distance)],
                             [{'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': h, 'state': {'attacker': 0, 'victim': 1}}] * 3, 0, table)
                case['rng'] = {'mathSeed': 4, 'ints': [first, (first + 1) % 3, 2, 0, 1, 5, 7, 9, 11, 13, 1, 2]}
                out.append((case, None))
    # Plain attacks with projectiles, a fixed maximum hit, a magic style attack and the style constructor.
    table = [{'kind': 'multi', 'attacks': [
        {'handler': 1, 'animation': 2989, 'start': [499, 0, 0], 'end': [502, 0, 0], 'projectile': MAGIC_TEMPLATE},
        {'handler': 2, 'animation': 81, 'projectile': MAGIC_TEMPLATE, 'maximumHit': 28},
        {'style': 'MELEE'},
        {'handler': 3, 'animation': 5, 'useHandler': False}]}]
    # Tiles apart: 0, 2, 3 (0.3 and 0.5 delay factors round apart), 5 and 8.
    for distance in (1, 3, 4, 6, 9):
        ops = []
        for _ in range(5):
            ops += [{'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': FIRST, 'state': {'attacker': 0, 'victim': 1}},
                    {'op': 'visualize', 'e': 0, 'v': 1, 'h': FIRST, 'state': {'attacker': 0, 'victim': 1}},
                    {'op': 'visualizeImpact', 'e': 0, 'v': 1, 'h': FIRST, 'state': {'attacker': 0, 'victim': 1}},
                    {'op': 'impact', 'e': 0, 'v': 1, 'h': FIRST, 'state': {'attacker': 0, 'victim': 1, 'estimatedHit': 3, 'targets': ['this', {'attacker': 0, 'victim': 1, 'estimatedHit': 6}]}},
                    {'op': 'hit', 'e': 0, 'v': 1, 'h': FIRST, 'modifier': hexd(1.0)}]
        out.append((world([dragon(), at(player(), distance)], ops, distance, table), 0 if distance == 3 else None))
    # A single melee attack drops melee distance; the vararg constructor keeps it; a null handler fails.
    for attacks in ([{'handler': 0, 'animation': 80}], [{'handler': 1}], [{'handler': None, 'animation': 3}],
                    [{'handler': 0}, {'handler': None}]):
        for flag in (None, True, False):
            spec = {'kind': 'multi', 'attacks': attacks}
            if flag is not None:
                spec['meleeDistance'] = flag
            ops = [{'op': 'canSwing', 'e': 0, 'v': 1, 'h': FIRST},
                   {'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': FIRST, 'state': {'attacker': 0, 'victim': 1}},
                   {'op': 'handlerSwing', 'e': 0, 'v': 1, 'h': FIRST, 'state': {'attacker': 0, 'victim': 1}},
                   {'op': 'impact', 'e': 0, 'v': 1, 'h': FIRST, 'state': {'attacker': 0, 'victim': 1, 'targets': [None]}}]
            if attacks[0].get('handler') is None:
                continue
            out.append((world([dragon(), at(player(), 2)], ops, 5, [spec]), None))
    return out


def mixed_cases():
    """The three MixedSwingHandlers objects: offence of their class, defence of another singleton."""
    out = []
    for h in (3, 4, 5):
        for seed in range(3):
            target = at(player(None), 1 if h != 3 else 4)
            ops = [{'op': 'defence', 'e': 1, 'v': 0, 'h': h}, {'op': 'accuracy', 'e': 0, 'h': h},
                   {'op': 'canSwing', 'e': 0, 'v': 1, 'h': h},
                   {'op': 'swing', 'e': 0, 'v': 1, 'h': h}, {'op': 'tick', 'n': 4}]
            out.append((world([dragon(handler=h), target], ops, seed), 3 if seed == 0 else None))
    return out


def fire_type_cases():
    """FireType: the four breaths' animation, projectile and effect."""
    out = []
    for kind in ('FIERY_BREATH', 'SHOCKING_BREATH', 'TOXIC_BREATH', 'ICY_BREATH'):
        for victim in (player(), dragon()):
            for others in ([1], [], [None]):
                ops = [{'op': 'fireType', 'e': 0, 'type': kind, 'others': others}]
                case = world([victim, dragon()], ops * 3, 0)
                # A first draw of 3 is the bound of the shock chance.
                case['rng'] = {'ints': [3, 1, 2, 2, 0, 5, 1]}
                out.append((case, 0 if (kind, others) == ('SHOCKING_BREATH', [1]) and victim['kind'] == 'player' else None))
        case = world([player(), dragon()], [{'op': 'fireType', 'e': 0, 'type': kind, 'others': [1]}], 0, nullTimers=['frozen'])
        out.append((case, None))
    return out


def dfs_player(charge=1000, **kw):
    spec = player(11283, charge, **kw)
    spec['inventory'] = {'0': [11283, 1, charge, 'Dragonfire shield'], '1': [11284, 1, 1000, 'Dragonfire shield']}
    return spec


def shield_cases():
    """DragonfireShieldListener: its four options and the anonymous handler's attack, and DFSItemPlugin."""
    out = []
    listen = [{'op': 'listen'}]
    # Inspect, empty and drop on both shields, from the inventory.
    for slot in (0, 1):
        for charge in (0, 19, 20, 999, 1000):
            ops = listen + [{'op': 'option', 'e': 0, 'option': o, 'slot': slot, 'where': 'inventory'} for o in ('inspect', 'empty', 'inspect', 'drop')]
            out.append((world([dfs_player(charge)], ops), 2 if (slot, charge) == (0, 1000) else None))
        out.append((world([dfs_player()], listen + [{'op': 'option', 'e': 0, 'option': 'drop', 'slot': slot, 'where': 'inventory'}]), None))
    # Operate: not charged, recharging, a prepared attack cancelled with and without the bar on.
    for charge, locked in [(19, False), (20, True), (20, False), (1000, False)]:
        for toggled in (False, True):
            p = dfs_player(charge, specialToggled=toggled, locked=['dfs_recharge'] if locked else [])
            ops = listen + [{'op': 'option', 'e': 0, 'option': 'operate', 'slot': 5, 'where': 'equipment'},
                            {'op': 'option', 'e': 0, 'option': 'operate', 'slot': 5, 'where': 'equipment'},
                            {'op': 'option', 'e': 0, 'option': 'operate', 'slot': 5, 'where': 'equipment'}]
            out.append((world([p, at(dragon(), 3)], ops, charge), 1 if (charge, locked, toggled) == (1000, False, True) else None))
    uncharged = player(11284)
    out.append((world([uncharged], listen + [{'op': 'option', 'e': 0, 'option': 'operate', 'slot': 5, 'where': 'equipment'}]), None))
    # The prepared attack replaces the next swing of a fight; at once when attacking within reach.
    for charge in (20, 39, 40, 1000):
        for better in (False, True):
            for distance in (2, 6, 14):
                p = dfs_player(charge)
                ops = listen + [{'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 1},
                                {'op': 'option', 'e': 0, 'option': 'operate', 'slot': 5, 'where': 'equipment'},
                                {'op': 'tick', 'n': 6}]
                sweep = {(39, False, 2): 3, (40, True, 2): 4}.get((charge, better, distance))
                out.append((world([p, at(dragon(), distance)], ops, charge + distance, betterDfs=better), sweep))
    for charge in (20, 1000):
        p = dfs_player(charge)
        ops = listen + [{'op': 'option', 'e': 0, 'option': 'operate', 'slot': 5, 'where': 'equipment'},
                        {'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 8}]
        out.append((world([p, at(dragon(), 4)], ops, charge), 2 if charge == 20 else None))
    # The handler swings when the shield was taken off or uncharged in between, and against a player.
    for change in ('none', 'uncharged', 'player'):
        p = dfs_player(1000)
        target = at(player(None), 3) if change == 'player' else at(dragon(), 3)
        ops = listen + [{'op': 'option', 'e': 0, 'option': 'operate', 'slot': 5, 'where': 'equipment'}]
        if change == 'uncharged':
            ops.append({'op': 'option', 'e': 0, 'option': 'empty', 'slot': 5, 'where': 'equipment'})
        ops += [{'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 7}]
        out.append((world([p, target], ops, 9), None))
    # The death item and the plugin registration.
    out.append((world([dfs_player()], [{'op': 'deathItem', 'e': 0, 'slot': 0, 'where': 'inventory'},
                                       {'op': 'deathItem', 'e': 0, 'slot': 5, 'where': 'equipment'}, {'op': 'itemPlugin'}]), None))
    return out


def fight_cases():
    """Whole fights: CombatPulse asks the dragon's handler, the runner delivers."""
    out = []
    for table in (chromatic(), metal()):
        for shield in (None, 11283, 1540):
            for distance in (2, 5):
                for seed in range(2):
                    victim = at(player(shield, 990, prayers=['PROTECT_FROM_MAGIC'] if seed else []), distance)
                    ops = [{'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 16}]
                    out.append((world([dragon(handler=FIRST + 1), victim], ops, seed * 5 + distance, table), None))
    # The wyvern's icy breath and KBD's handler-less dragonfire as an NPC's whole handler.
    for h, spec in [(FIRST, get(False, 54, False, start=[501, 0, 0])),
                    (FIRST, {'kind': 'dragonfire', 'via': 'new', 'meleeRange': True, 'maximumHit': 56, 'fire': True,
                             'attack': {'handler': None, 'animation': 81, 'end': [3, 0, 0]}})]:
        for shield in (2890, None):
            ops = [{'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 12}]
            out.append((world([dragon(handler=h), at(player(shield), 1)], ops, 2, [spec]), None))
    return out


def dragon_cases():
    """ChromaticDragonNPC, MetalDragonNPC and ChromaticDragonBehavior: their handlers, protection and egg drops."""
    out = []
    for kind, npc_id in [('chromatic', 941), ('chromatic', 55), ('chromatic', 8629), ('metal', 1590), ('metal', 3590)]:
        for shield in (None, 11283, 1540):
            # One and two tiles apart: a chromatic dragon only reaches in melee range.
            for distance in (2, 3):
                for seed in range(2):
                    first = dragon(id=npc_id, **{'class': kind})
                    victim = at(player(shield, 990, timers=['DragonFireImmunity'] if seed else []), distance)
                    ops = [{'op': 'dragonInfo', 'e': 0}, {'op': 'maxHit', 'e': 0, 'max': 52, 'wyvern': False, 'unprotectable': 0, 'message': True},
                           {'op': 'protection', 'e': 0, 'fire': True},
                           {'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 14}]
                    out.append((world([first, victim], ops, seed * 3 + distance + npc_id), None))
    # Swept: the first ticks of a metal dragon's fight, every call failing once in turn.
    out.append((world([dragon(id=1591, **{'class': 'metal'}), at(player(11283, 990), 3)],
                      [{'op': 'attack', 'e': 0, 'v': 1}, {'op': 'tick', 'n': 3}], 4), 1))
    # Two dragons of a class share its dragonfire; each has its own switching handler.
    for kind in ('chromatic', 'metal'):
        entities = [dragon(id=53, **{'class': kind}), at(dragon(id=54, **{'class': kind}), 0, 2), at(player(None), 1)]
        ops = [{'op': 'dragonInfo', 'e': 0}, {'op': 'dragonInfo', 'e': 1}, {'op': 'attack', 'e': 0, 'v': 2}, {'op': 'attack', 'e': 1, 'v': 2},
               {'op': 'tick', 'n': 10}]
        out.append((world(entities, ops, 8), None))
    # ChromaticDragonBehavior: eggs removed, the killer's summoning, the rate and the dragon's colour.
    for npc_id in (941, 4680, 55, 4684, 53, 54, 4676, 8629, 1590):
        for summoning in (98, 99):
            for draws in ([1], [0], ['max']):
                killer = player(None, staticLevels=dict(levels(70), **{'23': summoning}))
                drops = [536, 12477, 1753, 12480, 12478, 12479, 12477]
                ops = [{'op': 'dropTable', 'e': 0, 'v': 1, 'drops': drops}]
                case = world([dragon(id=npc_id), killer], ops, 0)
                case['rng'] = {'ints': draws}
                out.append((case, 0 if (npc_id, summoning, draws) == (53, 99, [1]) else None))
    for rate in (1, 2, 0, -3):
        ops = [{'op': 'dropTable', 'e': 0, 'v': 1, 'drops': [], 'eggRate': rate}, {'op': 'dropTable', 'e': 0, 'v': 1, 'drops': [12479]}]
        case = world([dragon(id=4677), player(None, staticLevels=levels(99))], ops, 0)
        case['rng'] = {'ints': [1, 1, 1]}
        out.append((case, None))
    return out


def cases():
    planned = []
    for group in [protection_cases, dragonfire_direct_cases, multi_cases, mixed_cases, fire_type_cases, shield_cases, fight_cases,
                  dragon_cases]:
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
