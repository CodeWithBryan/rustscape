"""The consumable corpus: every Consumables row eaten or drunk through the listener, the clocks, the
consume paths of every class (doses, halves, cakes, stacks, empty containers, stale slots), every effect
class with edge arguments, health values, names and doses, and random sequences.

`cases()` yields (case, sweep): when sweep is an operation index, generate.py adds one case per recorded
call of that operation in which that call fails.
"""
import pathlib
import random
import re

SERVER = pathlib.Path(__file__).resolve().parents[2] / 'reference/server/Server/src/main'

NEEDED_EVENTS = ['animate', 'audio', 'packet_message', 'message', 'manual_hit', 'chat', 'teleport', 'run_pulse', 'finish_task',
                 'familiar', 'special_points', 'replace', 'remove', 'remove_slot', 'contains', 'player_log', 'spawn_timer',
                 'register_timer', 'remove_timer', 'get_timer', 'remove_timer_class', 'active_script', 'add_to_queue', 'reset_scripts',
                 'log', 'delay_next_attack', 'skill_packet', 'perk', 'extension', 'run_energy_packet', 'next_int', 'ticks',
                 'get_attribute', 'set_attribute', 'on']


def rows():
    text = (SERVER / 'content/data/consumables/Consumables.java').read_text(encoding='utf-8')
    found = []
    for m in re.finditer(r'^\t([A-Z_]+)\(new (\w+)\((?:new\s+int\s*\[\]\s*\{([^}]*)\}|(\d+))', text, re.M):
        ids = [int(x) for x in re.findall(r'\d+', m.group(3))] if m.group(3) is not None else [int(m.group(4))]
        found.append((m.group(1), m.group(2), ids))
    nettle = [r for r in found if r[0] == 'NETTLE_WATER']
    if not nettle:
        # NETTLE_WATER names its ids through ConstLib (Items.NETTLE_WATER_4237, Items.BOWL_1923).
        found.append(('NETTLE_WATER', 'Drink', [4237, 1923]))
    oomlie = [r for r in found if r[0] == 'OOMLIE_WRAP']
    if not oomlie:
        found.append(('OOMLIE_WRAP', 'Food', [2343]))
    if len(found) < 300:
        raise RuntimeError(f'only {len(found)} Consumables rows parsed')
    return found


ROWS = rows()
KIND = {name: kind for name, kind, _ in ROWS}
IDS = {name: ids for name, _, ids in ROWS}


def pretty(name):
    return name.replace('_', ' ').lower().capitalize()


def names():
    out = {}
    for name, kind, ids in ROWS:
        base = pretty(name)
        for i, id in enumerate(ids):
            if str(id) in out:
                continue
            if kind == 'Potion':
                out[str(id)] = f'{base} potion({len(ids) - i})'
            elif kind == 'BarbarianMix':
                out[str(id)] = f'{base}({len(ids) - i})'
            elif kind == 'HalfableFood':
                if i == 0:
                    out[str(id)] = base
                elif i == 1:
                    out[str(id)] = ('1/2 ' + base.lower()) if 'pizza' in base.lower() else ('Half a ' + base.lower())
                else:
                    out[str(id)] = 'Pie dish'
            elif kind == 'Drink' and len(ids) == 5:
                out[str(id)] = f'{base}(m{4 - i})' if name.endswith('_M_KEG') and i < 4 else (f'{base}({4 - i})' if i < 4 else 'Empty keg')
            elif i == len(ids) - 1 and len(ids) > 1 and kind in ('Food', 'Drink'):
                out[str(id)] = {1923: 'Bowl', 1919: 'Beer glass', 2026: 'Cocktail glass', 1935: 'Jug', 1980: 'Empty cup', 7742: 'Beer glass',
                                7921: 'Empty wine bottle', 4244: 'Porcelain cup', 7728: 'Empty cup', 7732: 'Empty cup', 7735: 'Empty cup',
                                1925: 'Bucket', 3805: 'Tankard'}.get(id, base)
            else:
                out[str(id)] = base
    out['229'] = 'Vial'
    return out


NAMES = names()
EAT = {'Food', 'Cake', 'HalfableFood', 'FakeConsumable'}

STATIC = {str(i): 75 for i in range(24)}
STATIC.update({'3': 82, '5': 61, '6': 48, '23': 66})
DYNAMIC = {str(i): 75 for i in range(24)}
DYNAMIC.update({'0': 70, '1': 78, '2': 60, '3': 82, '4': 75, '5': 55, '6': 51, '10': 71, '16': 80, '18': 74, '22': 75, '23': 60})


def player(**extra):
    p = dict(static=dict(STATIC), dynamic=dict(DYNAMIC), lp=40, prayer=30.0, energy=50.0)
    p.update(extra)
    return p


def case(group, ops, tick=100, seed=0, **extra):
    c = dict(group=group, tick=tick, seed=seed, ops=ops)
    c['player'] = player(**extra.pop('player', {}))
    c.update(extra)
    # The definition names of the items the case holds or names (the only names a message can read).
    used = {str(r[1]) for key in ('inventory', 'equipment') for r in c['player'].get(key, []) if r[1] is not None}
    used |= {str(op['item'][0]) for op in ops if 'item' in op}
    c['names'] = {k: NAMES[k] for k in sorted(used) if k in NAMES}
    return c


def option(row):
    return 'eat' if KIND[row] in EAT else 'drink'


def holding(id, amount=1, slot=5, option_name='eat', **extra):
    p = dict(inventory=[[slot, id, amount]], attrs={'interact:option': option_name})
    p.update(extra)
    return p


SWEPT_ROWS = {
    'SHARK', 'COOKED_KARAMBWAN', 'POISON_KARAMBWAN', 'THIN_SNAIL', 'SPIDER_ON_STICK', 'STUFFED_SNAKE', 'SEAWEED_SANDWICH', 'UGTHANKI_KEBAB',
    'UGTHANKI_KEBAB_SMELLING', 'KEBAB', 'SUPER_KEBAB', 'REDBERRY_PIE', 'GARDEN_PIE', 'WILD_PIE', 'SUMMER_PIE', 'STEW', 'PLAIN_PIZZA', 'CAKE',
    'ROCK_CAKE', 'DWARVEN_ROCK_CAKE', 'MINT_CAKE', 'SWEET_CORN', 'DRAYNOR_CABBAGE', 'EVIL_TURNIP', 'CHOCOLATEY_MILK', 'JANGERBERRIES',
    'STRAWBERRY', 'WATERMELON_SLICE', 'WHITE_TREE_FRUIT', 'STRANGE_FRUIT', 'ASGARNIAN_ALE', 'ASGARNIAN_ALE_KEG', 'BEER', 'KEG_OF_BEER',
    'CHEFS_DELIGHT', 'CIDER', 'WIZARDS_MIND_BOMB', 'MATURE_WMB', 'PINEAPPLE_PUNCH', 'BRAINDEATH_RUM', 'RUM_TROUBLE_BREWING_RED', 'GIN',
    'CUP_OF_TEA', 'PURPLE_SWEETS_STACKABLE', 'OOMLIE_WRAP', 'FROG_SPAWN', 'STRENGTH', 'MAGIC', 'ANTIPOISON', 'RELICYM', 'RESTORE', 'SARA_BREW',
    'SUMMONING', 'ENERGY', 'PRAYER', 'SUPER_RESTO', 'ZAMMY_BREW', 'ANTIFIRE', 'GUTH_REST', 'SANFEW', 'BLAMISH_OIL', 'PRAYERMIX', 'ZAMMY_MIX',
    'RELIC_MIX', 'SUPER_RESTO_MIX', 'ANTIFIRE_MIX', 'SC_PRAYER', 'SC_SUMMONING', 'COOKED_CRAB_MEAT', 'CHOCOLATE_BOMB', 'SLAYERS_RESPITE_M_KEG',
}


def every_row():
    """Each row's every distinct id, eaten or drunk through the listener from a full inventory slot."""
    for n, (name, kind, ids) in enumerate(ROWS):
        seen = []
        for i, id in enumerate(ids):
            if id in seen:
                continue
            seen.append(id)
            amount = 3 if len(ids) == 1 and name == 'PURPLE_SWEETS_STACKABLE' else 1
            extra = {}
            if name in ('RELICYM', 'RELIC_MIX'):
                extra['timers'] = [['disease', 12]]
            if name in ('SUMMONING', 'SC_SUMMONING', 'SUPER_RESTO_MIX'):
                extra['familiar'] = True
            ops = [{'op': 'handle', 'slot': 5}]
            if kind == 'Potion':
                ops.append({'op': 'script', 'index': 0})
            if name.startswith('RUM_TROUBLE_BREWING'):
                ops += [{'op': 'pulse', 'index': 0}, {'op': 'pulse', 'index': 1}]
            sweep = 0 if name in SWEPT_ROWS and i == 0 else None
            yield case('rows', ops, tick=200 + n, seed=n, player=holding(id, amount, option_name=option(name), **extra)), sweep


def clocks():
    """The listener's clock gates: food, karambwan, drinks, and their mixes over consecutive ticks."""
    shark, karambwan, potion, beer = 385, 3144, 113, 1917
    inv = [[0, shark, 1], [1, shark, 1], [2, karambwan, 1], [3, karambwan, 1], [4, potion, 1], [5, beer, 1], [6, 2140, 1], [7, 3146, 1]]
    def eat(slot):
        return [{'op': 'set', 'fields': {'attrs': {'interact:option': 'eat'}}}, {'op': 'handle', 'slot': slot}]
    def drink(slot):
        return [{'op': 'set', 'fields': {'attrs': {'interact:option': 'drink'}}}, {'op': 'handle', 'slot': slot}]
    sequences = [
        eat(0) + eat(1),
        eat(0) + [{'op': 'tick', 'value': 101}] + eat(1) + [{'op': 'tick', 'value': 102}] + eat(1) + [{'op': 'tick', 'value': 103}] + eat(1),
        eat(0) + eat(2) + eat(3),
        eat(2) + eat(0) + [{'op': 'tick', 'value': 103}] + eat(0),
        eat(2) + eat(3) + [{'op': 'tick', 'value': 102}] + eat(3) + [{'op': 'tick', 'value': 103}] + eat(3),
        drink(4) + drink(5) + [{'op': 'tick', 'value': 101}] + drink(5) + [{'op': 'tick', 'value': 102}] + drink(5),
        drink(4) + eat(0) + eat(2),
        eat(0) + drink(4) + drink(5),
        eat(6) + eat(7) + [{'op': 'tick', 'value': 103}] + eat(7),
        eat(2) + drink(4) + [{'op': 'tick', 'value': 101}] + drink(5),
    ]
    for k, ops in enumerate(sequences):
        yield case('clocks', ops, player=dict(inventory=inv)), None
    # Clock values exactly at, one below and one above the tick for each gate and option.
    for clock in (2, 3, 4):
        for delta in (-1, 0, 1):
            for slot, opt in ((0, 'eat'), (2, 'eat'), (4, 'drink')):
                fields = {'clocks': {str(clock): 100 + delta}, 'attrs': {'interact:option': opt}, 'inventory': inv}
                yield case('clock-edges', [{'op': 'handle', 'slot': slot}], player=fields), None
    # Clocks near the integer limits.
    for value in (2147483647, -2147483648):
        for slot, opt in ((0, 'eat'), (4, 'drink'), (2, 'eat')):
            fields = {'clocks': {'2': value, '3': value, '4': value}, 'attrs': {'interact:option': opt}, 'inventory': inv}
            yield case('clock-edges', [{'op': 'handle', 'slot': slot}], tick=2147483646, player=fields), None


def options_and_nodes():
    inv = [[0, 385, 1], [1, 113, 1], [2, 3144, 1]]
    for value in ['eat', 'drink', 'Eat', 'DRINK', '', 'eat ', None, 5, True]:
        attrs = {} if value is None else {'interact:option': value}
        for slot in (0, 1, 2):
            yield case('options', [{'op': 'handle', 'slot': slot}], player=dict(inventory=inv, attrs=attrs)), (0 if value == 5 and slot == 0 else None)
    for node in [{'other': 385}, {'other': 113}, {'other': 3144}, {'other': 995}, {'item': [995, 1, 0]}, {'item': [385, 1, 9]},
                 {'item': [385, 1, 0]}, {'item': [385, 2, 0]}, {'item': [3144, 1, 27]}, {'item': [113, 1, 1]}, {'other': 0}]:
        for opt in ('eat', 'drink'):
            yield case('nodes', [dict(node, op='handle')], player=dict(inventory=inv, attrs={'interact:option': opt})), None
    yield case('define', [{'op': 'define'}], player={}), 0


def consume_paths():
    """Direct consume calls: stale and mismatched slots, stacks, the 65535 id, the last ids, foreign rows and slot bounds."""
    specs = [
        ('SHARK', [385, 1, 4], [[4, 385, 1]], []),
        ('SHARK', [385, 1, 4], [[4, 379, 1]], []),
        ('SHARK', [385, 1, 4], [], [[3, 385, 1]]),
        ('SHARK', [385, 1, 4], [], []),
        ('SHARK', [385, 2, 4], [[4, 385, 2]], []),
        ('SHARK', [385, 2, 4], [[4, 385, 1]], [[3, 385, 5]]),
        ('SHARK', [385, 0, 4], [[4, 385, 1]], []),
        ('SHARK', [385, -3, 4], [[4, 385, 1]], []),
        ('SHARK', [385, 1, 28], [], []),
        ('SHARK', [385, 1, -1], [], []),
        ('PURPLE_SWEETS_STACKABLE', [10476, 5, 2], [[2, 10476, 5]], []),
        ('PURPLE_SWEETS_STACKABLE', [10476, 1, 2], [[2, 10476, 1]], []),
        ('PURPLE_SWEETS_STACKABLE', [10476, 5, 2], [[2, 10476, 4]], []),
        ('BEER', [1917, 1, 3], [[3, 1917, 1]], []),
        ('BEER', [1919, 1, 3], [[3, 1919, 1]], []),
        ('STEW', [1923, 1, 3], [[3, 1923, 1]], []),
        ('STEW', [2003, 1, 3], [[3, 2003, 1]], []),
        ('REDBERRY_PIE', [2313, 1, 3], [[3, 2313, 1]], []),
        ('REDBERRY_PIE', [2333, 1, 3], [[3, 2333, 1]], []),
        ('PLAIN_PIZZA', [2291, 1, 3], [[3, 2291, 1]], []),
        ('PLAIN_PIZZA', [2289, 1, 3], [[3, 2289, 1]], []),
        ('CAKE', [1891, 1, 3], [[3, 1891, 1]], []),
        ('CAKE', [1893, 1, 3], [[3, 1893, 1]], []),
        ('CAKE', [1895, 1, 3], [[3, 1895, 1]], []),
        ('CAKE', [385, 1, 3], [[3, 385, 1]], []),
        ('CHOCOLATE_CAKE', [1901, 1, 3], [[3, 1901, 1]], []),
        ('STRENGTH', [113, 1, 3], [[3, 113, 1]], []),
        ('STRENGTH', [119, 1, 3], [[3, 119, 1]], []),
        ('STRENGTH', [117, 1, 3], [[3, 117, 1]], []),
        ('STRENGTH', [385, 1, 3], [[3, 385, 1]], []),
        ('STRENGTH', [229, 1, 3], [[3, 229, 1]], []),
        ('SC_PRAYER', [14215, 1, 3], [[3, 14215, 1]], []),
        ('PRAYERMIX', [11467, 1, 3], [[3, 11467, 1]], []),
        ('PRAYERMIX', [11465, 1, 3], [[3, 11465, 1]], []),
        ('SHARK', [379, 1, 3], [[3, 379, 1]], []),
        ('SHARK', [995, 1, 3], [[3, 995, 1]], []),
        ('VEG_BALL', [2195, 1, 3], [[3, 2195, 1]], []),
        ('CHOCOLATE_BOMB', [2195, 1, 3], [[3, 2195, 1]], []),
        ('DWARVEN_ROCK_CAKE', [7510, 1, 3], [[3, 7510, 1]], []),
        ('HOT_DWARVEN_ROCK_CAKE', [7509, 1, 3], [[3, 7509, 1]], []),
        ('SEAWEED_SANDWICH', [3168, 1, 3], [[3, 3168, 1]], []),
        ('SEAWEED_SANDWICH', [385, 1, 3], [], []),
        ('COOKED_CRAB_MEAT', [7526, 1, 3], [[3, 7526, 1]], []),
        ('SPIDER_ON_STICK', [6305, 1, 3], [[3, 6305, 1]], []),
        ('KEG_OF_BEER', [3801, 1, 3], [[3, 3801, 1]], []),
        ('ASGARNIAN_ALE_KEG', [5769, 1, 3], [[3, 5769, 1]], []),
        ('EVIL_TURNIP', [12138, 1, 3], [[3, 12138, 1]], []),
        ('RUM_TROUBLE_BREWING_BLUE', [8941, 1, 3], [[3, 8941, 1]], []),
        ('SHARK', [65535 + 385 + 1, 1, 3], [[3, 385, 1]], []),
    ]
    for k, (row, item, inventory, equipment) in enumerate(specs):
        ops = [{'op': 'consume', 'row': row, 'item': item}]
        if KIND[row] == 'Potion':
            ops.append({'op': 'script', 'index': 0})
        sweep = 0 if k in (1, 2, 5, 13, 22, 28, 33) else None
        yield case('consume', ops, seed=k, player=dict(inventory=inventory, equipment=equipment)), sweep


def dose_scripts():
    """Potion dose messages: doses left 3, 2, 1, 0, a five-dose potion, a persistent active script and a second stage."""
    for row, ids in (('ATTACK', IDS['ATTACK']), ('SC_ENERGY', IDS['SC_ENERGY'])):
        for i, id in enumerate(ids):
            for active in (None, False, True):
                extra = {} if active is None else {'active': active}
                ops = [{'op': 'consume', 'row': row, 'item': [id, 1, 0]}, {'op': 'script', 'index': 0}, {'op': 'script', 'index': 0, 'stage': 3}]
                yield case('doses', ops, player=dict(inventory=[[0, id, 1]], **extra)), (1 if active and i == 0 and row == 'ATTACK' else None)


def effects():
    """Every effect class, alone, with edge arguments and player states."""
    E = lambda cls, *args: {'class': cls, 'args': list(args)}
    states = [
        {},
        dict(lp=82),
        dict(lp=1),
        dict(lp=0),
        dict(lp=2),
        dict(lp=3),
        dict(lp=6),
        dict(lp=90, sponge=True),
        dict(lp=5, prayer=0.0, energy=0.0),
        dict(lp=82, prayer=61.0, energy=100.0),
        dict(dynamic={str(i): 99 for i in range(24)}),
        dict(dynamic={str(i): 1 for i in range(24)}, prayer=1.0),
        dict(static={str(i): 99 for i in range(24)}, dynamic={str(i): 30 for i in range(24)}, lp=50, prayer=12.5),
        dict(inventory=[[0, 6714, 1]], prayer=10.0),
        dict(familiar=True),
        dict(timers=[['disease', 9]]),
        dict(timers=[['disease', 10]]),
        dict(timers=[['disease', 25], ['poison']]),
        dict(timers=[['disease', -5]]),
        dict(timers=[['poison'], ['poison:immunity']]),
        dict(lp=2147483647),
        dict(lp=-2147483648),
    ]
    effects_list = [
        E('AchievementEffect', 'KARAMJA', 2, 2), E('AchievementEffect', 'SEERS_VILLAGE', 0, 7),
        E('AddTimerEffect', 'poison:immunity', 150), E('AddTimerEffect', 'dragonfire:immunity', 600, True), E('AddTimerEffect', 'unknown'),
        E('CureDiseaseEffect'),
        E('DamageEffect', 10.0, True, 2), E('DamageEffect', 1.0, False), E('DamageEffect', -3.0, False), E('DamageEffect', 2.5, False, 1),
        E('DamageEffect', 33.3, True), E('DamageEffect', 0.0, True, -4),
        E('DraynorCabbageEffect'), E('DwarvenRockCakeEffect'), E('EnergyEffect', 10), E('EnergyEffect', -30), E('EnergyEffect', 250),
        E('HealingEffect', 20), E('HealingEffect', 0), E('HealingEffect', -7), E('HealingEffect', 2147483647),
        E('KebabEffect'), E('KegOfBeerEffect'), E('MatureWmbEffect'),
        E('MultiEffect', E('HealingEffect', 1), E('PercentageHealthEffect', 10)), E('MultiEffect'),
        E('MultiEffect', E('MultiEffect', E('HealingEffect', 2), E('SkillEffect', 19, 1.0, 0.0)), E('EnergyEffect', 5)),
        E('PercentageHealthEffect', 10), E('PercentageHealthEffect', 6), E('PercentageHealthEffect', 150), E('PercentageHealthEffect', -20),
        E('PercentHeal', 2, 0.15), E('PercentHeal', 0, 1.5), E('PercentHeal', -10, 0.0),
        E('PoisonEffect', 5), E('PoisonEffect', -2), E('PoisonKarambwanEffect'),
        E('PrayerEffect', 7.0, 0.25), E('PrayerEffect', 0.0, 0.10, True), E('PrayerEffect', -5.0, 0.0), E('PrayerEffect', 0.0, 0.0),
        E('PrayerEffect', -2.0, 0.1, True), E('PrayerEffect', 8.0, 0.25, False),
        E('RandomEnergyEffect', 5, 10), E('RandomEnergyEffect', 10, 5), E('RandomEnergyEffect', 3, 3),
        E('RandomHealthEffect', 1, 3), E('RandomHealthEffect', -4, 0), E('RandomHealthEffect', 0, 0), E('RandomHealthEffect', 7, -2),
        E('RandomPrayerEffect', 1, 5), E('RandomPrayerEffect', -3, -1),
        E('RandomSkillEffect', 0, 1, 3), E('RandomSkillEffect', 23, 3, 1), E('RandomSkillEffect', 24, 1, 2),
        E('RemoveTimerEffect', 'poison'), E('RemoveTimerEffect', 'disease'), E('RemoveTimerEffect', 'nothing'),
        E('RestoreEffect', 10.0, 0.3), E('RestoreEffect', 8.0, 0.25, True), E('RestoreEffect', -50.0, 0.0, True), E('RestoreEffect', 0.0, 0.0, False),
        E('RestoreSummoningSpecial'), E('RockCakeEffect'),
        E('SetAttributeEffect', 'drunk', 50), E('SetAttributeEffect', 'flag', True), E('SetAttributeEffect', 'name', 'value'),
        E('SetAttributeEffect', 'ticks', 'text', True), E('SetAttributeEffect', 'plain', 7, False), E('SetAttributeEffect', 'big', 2147483647),
        E('SkillEffect', 2, 3.0, 0.1), E('SkillEffect', 0, -4.0, 0.0), E('SkillEffect', 1, 0.0, -0.1), E('SkillEffect', 5, 2.0, 0.2),
        E('SkillEffect', 3, 5.0, 0.0), E('SkillEffect', 6, -100.0, 0.0), E('SkillEffect', 24, 1.0, 0.0), E('SkillEffect', -1, 1.0, 0.0),
        E('SkillEffect', 7, 0.0, 0.0), E('SkillEffect', 8, 1e10, 0.0),
        E('SmellingUgthankiKebabEffect'), E('SummoningEffect', 7.0, 0.25), E('SummoningEffect', -3.0, 0.0),
        E('SuperKebabEffect'), E('TroubleBrewingRumEffect', 'Arr!'), E('UgthankiKebabEffect'), E('WizardsMindBombEffect'), E('ZamorakMixEffect'),
    ]
    k = 0
    for e in effects_list:
        for s, state in enumerate(states):
            if (k + s) % 3 and s not in (0, 2, 7, 13, 14, 15):
                continue
            ops = [{'op': 'effect', 'effect': e}, {'op': 'effectHealth', 'effect': e}]
            if e['class'] == 'TroubleBrewingRumEffect':
                ops += [{'op': 'pulse', 'index': 0}, {'op': 'pulse', 'index': 1}, {'op': 'pulse', 'index': 1}]
            yield case('effects', ops, seed=k * 31 + s, player=state), (0 if s == 0 and k % 4 == 0 else None)
        k += 1
    for e in effects_list:
        yield case('describe', [{'op': 'describe', 'effect': e}], player={}), None
    # spawnTimer answering null (an identifier the registry does not know).
    for row, id in (('ANTIPOISON', 2446), ('ANTIFIRE', 2452), ('SANFEW', 10925), ('ANTIP_MIX', 11433)):
        ops = [{'op': 'consume', 'row': row, 'item': [id, 1, 0]}]
        c = case('unknown-timers', ops, player=dict(inventory=[[0, id, 1]], timers=[['poison:immunity'], ['disease', 3]]))
        c['unknownTimers'] = ['poison:immunity', 'dragonfire:immunity']
        yield c, (0 if row == 'SANFEW' else None)


def randoms():
    """The random effects over many seeds, through the rows that use them."""
    for row, id in (('KEBAB', 1971), ('SUPER_KEBAB', 4608), ('UGTHANKI_KEBAB_SMELLING', 1885), ('THIN_SNAIL', 3369), ('GIN', 2019),
                    ('WHITE_TREE_FRUIT', 6469), ('PURPLE_SWEETS_STACKABLE', 10476)):
        for seed in range(40 if row in ('KEBAB', 'SUPER_KEBAB') else 12):
            ops = [{'op': 'consume', 'row': row, 'item': [id, 1, 0]}, {'op': 'health', 'row': row}]
            yield case('randoms', ops, seed=seed * 7919 + 13, player=dict(inventory=[[0, id, 1]], lp=30 + seed % 50)), None


def health_and_names():
    """getHealthEffectValue of every row, getFormattedName and Potion.getDose."""
    for n, (name, kind, ids) in enumerate(ROWS):
        yield case('health', [{'op': 'health', 'row': name}], seed=n, player=dict(lp=20 + n % 60)), None
    samples = ['Attack potion(4)', 'Attack potion (3)', 'Ale(m4)', 'Ale(m)', 'Beer (m2) ', '\tCider(1)\n', 'Half a redberry pie', 'Half an apple pie',
               '1/2 plain pizza', 'MiXeD CaSe(2)', '(4)(3)(2)(1)', '', '  ', 'Half a(4)', 'Cake 1/2(m1)', 'Stew ', 'Éclair(2)']
    for row in ('SHARK', 'BEER', 'STRENGTH', 'REDBERRY_PIE', 'CAKE', 'PRAYERMIX', 'SEAWEED_SANDWICH'):
        for k, text in enumerate(samples):
            yield dict(case('names', [{'op': 'formatted', 'row': row, 'item': [60000, 1, 0]}], player={}), names={'60000': text}), None
    for row in ('STRENGTH', 'SC_ATTACK', 'PRAYERMIX', 'ANTIPOISON'):
        for id in IDS[row] + [229]:
            yield case('doses', [{'op': 'dose', 'row': row, 'item': [id, 1, 0]}], player={}), None
        for text in ['Odd potion(3)', 'Odd potion', 'Potion 1/2', 'Potion (2.5)', 'Potion(-1)', 'Potion(99999999999)', 'Potion(+7)', '7']:
            yield dict(case('doses', [{'op': 'dose', 'row': row, 'item': [60001, 1, 0]}], player={}), names={'60001': text}), None


def table():
    lookups = sorted({id for _, _, ids in ROWS for id in ids} | {0, -1, 229, 995, 65535, 2195, 5841})
    ops = [{'op': 'table'}, {'op': 'potions'}] + [{'op': 'lookup', 'id': id} for id in lookups]
    yield case('table', ops, player={}), None


def sequences():
    """Random sequences over a mixed inventory: options, ticks, clocks and repeated consumption."""
    pool = [385, 3144, 3146, 113, 115, 2428, 1917, 1971, 4608, 2325, 1891, 2289, 7510, 3801, 6685, 3024, 2450, 11465, 10476, 1885, 2379,
            464, 2003, 7919, 1993, 9739, 12140, 3168, 11521]
    for seed in range(60):
        r = random.Random(seed)
        inventory = [[slot, r.choice(pool), 1] for slot in range(28)]
        tick = 1000
        ops = []
        for _ in range(r.randint(4, 14)):
            roll = r.random()
            if roll < 0.15:
                tick += r.randint(0, 3)
                ops.append({'op': 'tick', 'value': tick})
            elif roll < 0.25:
                ops.append({'op': 'set', 'fields': {'attrs': {'interact:option': r.choice(['eat', 'drink'])}}})
            elif roll < 0.3:
                ops.append({'op': 'set', 'fields': {'lp': r.randint(0, 90), 'energy': float(r.randint(0, 100))}})
            else:
                ops.append({'op': 'handle', 'slot': r.randrange(28)})
        extra = dict(inventory=inventory, attrs={'interact:option': r.choice(['eat', 'drink'])}, familiar=r.random() < 0.5,
                     sponge=r.random() < 0.2, lp=r.randint(1, 82))
        c = case('sequences', ops, tick=1000, seed=seed, player=extra)
        # A handle on an emptied slot is not an item node: replace it with a stale detached item.
        yield c, None


def java_first(seed, bound):
    """java.util.Random(seed).nextInt(bound), the first draw."""
    mask = (1 << 48) - 1
    state = (seed ^ 0x5DEECE66D) & mask
    while True:
        state = (state * 0x5DEECE66D + 0xB) & mask
        bits = state >> 17
        if bound & (bound - 1) == 0:
            return (bound * bits) >> 31
        value = bits % bound
        if bits - value + (bound - 1) < (1 << 31):
            return value


def boundaries():
    """The random effects at their outcome boundaries, the holy wrench with every prayer row, and brew overheals."""
    for row, id, bound, wanted in (('KEBAB', 1971, 100, [0, 65, 66, 86, 87, 95, 96, 99]), ('SUPER_KEBAB', 4608, 8, [4, 5]),
                                   ('UGTHANKI_KEBAB_SMELLING', 1885, 100, [9, 10])):
        for w in wanted:
            seed = next(s for s in range(100000) if java_first(s, bound) == w)
            for lp in (30, 82):
                ops = [{'op': 'consume', 'row': row, 'item': [id, 1, 0]}]
                yield case('boundaries', ops, seed=seed, player=dict(inventory=[[0, id, 1]], lp=lp)), None
    for row in ('PRAYER', 'SC_PRAYER', 'PRAYERMIX', 'SUPER_RESTO_MIX', 'ZAMMY_BREW', 'JANGERBERRIES', 'ZAMMY_MIX'):
        id = IDS[row][0]
        for level, points in ((1, 0.0), (40, 12.0), (61, 30.0), (99, 98.5)):
            for wrench in (False, True):
                inv = [[0, id, 1]] + ([[1, 6714, 1]] if wrench else [])
                fields = dict(inventory=inv, static=dict(STATIC, **{'5': level}), dynamic=dict(DYNAMIC, **{'5': level}), prayer=points)
                yield case('wrench', [{'op': 'consume', 'row': row, 'item': [id, 1, 0]}], player=fields), None
    for lp in (0, 1, 50, 82, 90, 94, 95, 120):
        for sponge in (False, True):
            fields = dict(inventory=[[0, 6685, 1]], lp=lp, sponge=sponge)
            yield case('overheal', [{'op': 'consume', 'row': 'SARA_BREW', 'item': [6685, 1, 0]}], player=fields), None


def cases():
    for generator in (table, options_and_nodes, every_row, clocks, consume_paths, dose_scripts, effects, randoms, boundaries, health_and_names,
                      sequences):
        yield from generator()


def groups(all_cases):
    out = {}
    for c in all_cases:
        out[c['group']] = out.get(c['group'], 0) + 1
    return dict(sorted(out.items()))
