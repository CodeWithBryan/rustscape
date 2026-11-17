"""Reproduce original combat swing calculations and swing state assembly.

The four swing handlers, BattleState, CombatStyle, InteractionType and the
equipment data classes they read are compiled unmodified on JDK 11. Everything
they reach outside that slice is a recording stand-in from adapters.txt.
Java/Kotlin are test oracles only; --check never rewrites fixtures.
"""
import argparse
import hashlib
import importlib.util
import itertools
import json
import os
from pathlib import Path
import random
import re
import struct
import subprocess
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
RUN = ROOT / 'runs/combat-swing-oracle'
SOURCE = ROOT / 'reference/server/Server/src/main'
COMBAT = 'core/game/node/entity/combat/'
spec = importlib.util.spec_from_file_location('compiler', ROOT / 'oracle/login/generate.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

UNMODIFIED = [
    COMBAT + 'CombatSwingHandler.kt', COMBAT + 'MeleeSwingHandler.kt', COMBAT + 'RangeSwingHandler.kt',
    COMBAT + 'MagicSwingHandler.kt', COMBAT + 'BattleState.java', COMBAT + 'CombatStyle.java',
    COMBAT + 'InteractionType.java', COMBAT + 'equipment/Weapon.java', COMBAT + 'equipment/RangeWeapon.java',
    COMBAT + 'equipment/Ammunition.java', COMBAT + 'equipment/ArmourSet.java', COMBAT + 'equipment/BoltEffect.java',
    'core/api/EquipmentSlot.kt',
]
SKILL = dict(ATTACK=0, DEFENCE=1, STRENGTH=2, HITPOINTS=3, RANGE=4, PRAYER=5, MAGIC=6, AGILITY=16, SLAYER=18)
STYLE_OF = dict(melee='MELEE', range='RANGE', magic='MAGIC')
FLAGS = ['IGNORE_STAT_BOOSTS_DAMAGE', 'IGNORE_STAT_BOOSTS_ACCURACY', 'IGNORE_PRAYER_BOOSTS_DAMAGE',
         'IGNORE_PRAYER_BOOSTS_ACCURACY', 'IGNORE_STAT_REDUCTION']
SETS = ['AHRIM', 'DHAROK', 'GUTHAN', 'KARIL', 'TORAG', 'VERAC', 'BERSERKER_NECKLACE_SETS']
UNDEAD = ['Aberrant spectre', 'Ankou', 'Banshee', 'Mighty banshee', 'Crawling Hand', 'Ghost', 'Ghast', 'Mummy',
          'Tarn', 'Revenant imp', 'Shade', 'Skeleton', 'skeleton', 'Skogre', 'Tortured Soul', 'Undead cow',
          'Zogre', 'Zombie', 'zombie', 'none', 'null', 'Man', 'ghost', 'Aberrant Spectre', 'crawling hand',
          'Zombi', 'anshee', '']


def H(value):
    return struct.pack('>d', float(value)).hex()


ONE = H(1.0)


def tables():
    """Attack styles and prayer bonuses read from the pinned text, never retyped."""
    read = c.read
    weapon = read(SOURCE / (COMBAT + 'equipment/WeaponInterface.java'))
    numbers = {name: int(value) for name, value in re.findall(r'public static final int ((?:BONUS|STYLE)_\w+) = (\d+);', weapon)}
    enum = c.method(weapon, 'public static enum WeaponInterfaces')
    interfaces = []
    for name, body in re.findall(r'^\t\t(\w+)\(Components\.\w+((?:, new AttackStyle\(\w+, \w+\))+)\)[,;]', enum, re.M):
        interfaces.append((name, [(numbers[a], numbers[b]) for a, b in re.findall(r'new AttackStyle\((\w+), (\w+)\)', body)]))
    assert len(interfaces) == 24, len(interfaces)
    prayer = read(SOURCE / 'core/game/node/entity/player/link/prayer/PrayerType.java')
    prayers = []
    for name, body in re.findall(r'^\t([A-Z_]+)\((.*)\)[,;]$', prayer, re.M):
        prayers.append((name, [(SKILL[s], float(v)) for s, v in re.findall(r'new SkillBonus\(Skills\.(\w+), ([0-9.]+)\)', body)]))
    assert len(prayers) == 27, len(prayers)
    return interfaces, prayers


def bonuses(values=None, **named):
    out = [0] * 15
    for index, value in (values or {}).items():
        out[index] = value
    return out


def levels(**named):
    return {str(SKILL[name.upper()]): value for name, value in named.items()}


def player(**fields):
    return dict(kind='player', **fields)


def npc(**fields):
    return dict(kind='npc', **fields)


def prayer_bonus(active):
    """Prayer.getSkillBonus's summation order over the active list."""
    out = {}
    for skill in sorted({s for _, row in active for s, _ in row}):
        total = 0.0
        for _, row in active:
            for s, value in row:
                if s == skill:
                    total += value
        out[str(skill)] = H(total)
    return out


def corpus(interfaces, prayers):
    rows = []

    def add(kind, entities, ops, flags=(), sweep=False, **extra):
        case = dict(handler=dict(kind=kind, flags=list(flags)), entities=entities, ops=ops, **extra)
        rows.append((case, sweep))

    def calc(kind, a=0, v=1, mod=ONE):
        style = STYLE_OF[kind]
        return [dict(op='accuracy', e=a), dict(op='hit', e=a, v=v, mod=mod), dict(op='defence', v=v, a=a),
                dict(op='defence', v=a, a=v), dict(op='accuracy', e=v), dict(op='hit', e=v, v=a, mod=mod),
                dict(op='threshold', e=a, v=v, style=style, accMod=ONE, defMod=ONE),
                dict(op='threshold', e=v, v=a, style=style, accMod=ONE, defMod=ONE)]

    add('melee', [], [dict(op='tables')])

    # Level, bonus and style grids. The same attacker/victim pair is read in both directions.
    grid_levels = [0, 1, 2, 50, 98, 99, 120]
    grid_bonus = [-65, -64, -1, 0, 1, 64, 137]
    for level in grid_levels:
        for bonus in grid_bonus:
            for style, slot in [(0, 0), (1, 1), (2, 2), (3, 0)]:
                attacker = player(levels=levels(attack=level, strength=level, defence=99 - min(level, 98)),
                                  bonuses=bonuses({slot: bonus, slot + 5: 20 - bonus, 11: bonus}), attackStyle=[style, slot])
                victim = npc(levels=levels(attack=level // 2 + 3, strength=level, defence=level),
                             bonuses=bonuses({slot: 7, slot + 5: bonus, 11: -bonus}), attackStyle=[0, slot])
                add('melee', [attacker, victim], calc('melee'))
            for style in [4, 5, 6]:
                attacker = player(levels=levels(range=level, defence=level), staticLevels=levels(range=99),
                                  bonuses=bonuses({4: bonus, 9: 20 - bonus, 14: bonus}), attackStyle=[style, 4])
                victim = npc(levels=levels(range=level, defence=120 - level), bonuses=bonuses({4: 9, 9: bonus, 14: -bonus}),
                             attackStyle=[4, 4])
                add('range', [attacker, victim], calc('range'))
            attacker = player(levels=levels(magic=level, defence=level), bonuses=bonuses({3: bonus, 8: 20 - bonus, 13: bonus, 14: -bonus}),
                              attackStyle=[8 if level % 2 else 7, 3])
            victim = npc(levels=levels(magic=level, defence=3), bonuses=bonuses({3: 5, 8: bonus, 13: bonus, 14: 4}))
            add('magic', [attacker, victim], calc('magic'))

    # Every attack style of every weapon interface, for a player on both sides.
    spread = [(i * 37) % 91 - 20 for i in range(15)]
    for name, styles in interfaces:
        for index, (style, slot) in enumerate(styles):
            for kind in (['melee', 'range', 'magic'] if index == 0 else ['melee']):
                attacker = player(levels=levels(attack=75, strength=80, defence=70, range=77, magic=82), bonuses=spread,
                                  attackStyle=[style, slot], name=name + str(index))
                victim = player(levels=levels(attack=60, strength=61, defence=62, range=63, magic=64), bonuses=spread[::-1],
                                attackStyle=styles[-1 - index])
                add(kind, [attacker, victim], calc(kind))

    # Each prayer alone, pairs and ordered triples. Sums follow the original accumulation.
    boosting = [p for p in prayers if p[1]]
    assert len(boosting) == 17
    rng = random.Random(53001)
    groups = [[p] for p in boosting] + [list(p) for p in itertools.combinations(boosting, 2)]
    groups += [rng.sample(boosting, 3) for _ in range(60)]
    for index, active in enumerate(groups):
        bonus = prayer_bonus(active)
        names = [p[0] for p in active]
        for level in ([1, 99, 118] if len(active) == 1 else [99 if index % 2 else 73]):
            for kind in (['melee', 'range', 'magic'] if len(active) == 1 else [['melee', 'range', 'magic'][index % 3]]):
                attacker = player(levels=levels(attack=level, strength=level, defence=level, range=level, magic=level),
                                  bonuses=spread, attackStyle=[index % 4, index % 3], prayerBonus=bonus, name='+'.join(names))
                victim = player(levels=levels(attack=level, strength=level, defence=level, range=level, magic=level),
                                bonuses=spread[::-1], attackStyle=[3, 2], prayerBonus=bonus)
                add(kind, [attacker, victim], calc(kind)[:4] + calc(kind)[6:7])

    # Set multipliers alone and combined.
    skills = [0, 1, 2, 3, 4, 6]
    for kind in ['melee', 'range', 'magic']:
        for who in ['player', 'npc', 'other', 'familiar']:
            for armour in [None] + SETS:
                for void in [[], ['MELEE'], ['RANGE'], ['MAGIC'], ['MELEE', 'RANGE', 'MAGIC']]:
                    if who != 'player' and void:
                        continue
                    entity = dict(kind=who, armourSet=armour, void=void, lifepoints=37, maxLifepoints=99)
                    add(kind, [entity], [dict(op='setMultiplier', e=0, skill=s) for s in skills])
        for maximum, current in [(99, 99), (99, 1), (99, 0), (10, 99), (120, 37), (2147483647, -1), (-2147483648, 1)]:
            for who in ['player', 'npc']:
                entity = dict(kind=who, armourSet='DHAROK', void=['MELEE'], lifepoints=current, maxLifepoints=maximum,
                              levels=levels(strength=99, attack=99), bonuses=bonuses({11: 100, 1: 80}), attackStyle=[1, 1])
                add(kind, [entity, npc()], [dict(op='setMultiplier', e=0, skill=s) for s in skills] + calc(kind)[:2])
        add(kind, [], [dict(op='setMultiplier', e=None, skill=s) for s in [0, 2, 4, 6]])

    # Perks, familiars, amulets, slayer tasks, flags and typed branches, one at a time.
    base = dict(levels=levels(attack=80, strength=85, defence=75, range=90, magic=88), staticLevels=levels(range=99),
                bonuses=spread, attackStyle=[2, 1], pulseVictim=1)
    target = dict(levels=levels(attack=40, strength=40, defence=45, range=40, magic=50), bonuses=spread[::-1], attackStyle=[3, 0])
    singles = [dict(perks=['PRECISION_STRIKES']), dict(perks=['ACCURATE_MARKSMAN']), dict(perks=['PRECISION_STRIKES', 'ACCURATE_MARKSMAN'])]
    singles += [dict(familiar=f) for f in [12822, 12790, 12089, 12786, 1, 'throw', 'manager', None]]
    singles += [dict(pulseVictim=None), dict(attackStyle=None), dict(bonuses=None), dict(bonuses=[1, 2, 3]), dict(bonuses=[5] * 12),
                dict(bonuses=[5] * 14), dict(attackStyle=[0, 14]), dict(attackStyle=[0, 10]), dict(attackStyle=[0, -1]), dict(attackStyle=[9, 0])]
    slain = []
    for amulet in [4081, 10588, 1704, None]:
        for slayer in [None, dict(ids=[100, 55, 1158]), dict(ids=[7]), dict(ids=[7], kalphites=True), dict(ids=[55], dragon=True)]:
            for rate in [1.0, 1.075, 1.1667]:
                for gloves in [[], ['12862']]:
                    if (slayer is None and (rate != 1.0 or gloves)) or (amulet == 1704 and rate == 1.075):
                        continue
                    row = dict(slayerTask=slayer, slayerBonus=H(rate), inEquipment=gloves)
                    if amulet:
                        row['equipment'] = {'2': [amulet, 1, 1000, 'Amulet']}
                    slain.append(row)
    for kind in ['melee', 'range', 'magic']:
        for change in singles:
            for who in ['player', 'npc']:
                attacker = dict(base, kind='player', **change)
                victim = dict(target, kind=who, id=55, name='Skeleton', familiar=change.get('familiar'))
                add(kind, [attacker, victim], calc(kind))
        # Amulet and slayer inputs only reach the melee handler; the others get a sample.
        for index, change in enumerate(slain):
            if kind != 'melee' and index % 17:
                continue
            for victim_id, name in [(55, 'Skeleton'), (1158, 'Kalphite Queen')]:
                attacker = dict(base, kind='player', **change)
                add(kind, [attacker, dict(target, kind='npc', id=victim_id, name=name)], calc(kind)[:2] + calc(kind)[6:7])
        for count in range(len(FLAGS) + 1):
            for flags in itertools.combinations(FLAGS, count):
                attacker = dict(base, kind='player', prayerBonus=prayer_bonus([boosting[-1], boosting[10], boosting[11]]),
                                levels=levels(attack=80, strength=85, defence=75, range=60, magic=88))
                add(kind, [attacker, dict(target, kind='npc')], calc(kind)[:4], flags=flags)
        for name in (UNDEAD if kind == 'melee' else UNDEAD[:2]):
            for amulet in [4081, 10588]:
                attacker = dict(base, kind='player', equipment={'2': [amulet, 1, 1000, 'Salve amulet']}, slayerTask=dict(ids=[55]),
                                slayerBonus=H(1.1667))
                add(kind, [attacker, dict(target, kind='npc', id=55, name=name)], calc(kind)[:2])
        add(kind, [dict(base, kind='player', equipment={'2': [4081, 1, 1000, 'Salve amulet']}), dict(target, kind='npc', name=None)], calc(kind)[:2])
        for a, v in itertools.product(['player', 'npc', 'familiar', 'other'], repeat=2):
            add(kind, [dict(base, kind=a), dict(target, kind=v, prayers=['PROTECT_FROM_SUMMONING'])], calc(kind))
        add(kind, [dict(base, kind='player'), dict(target, kind='npc')], [dict(op='accuracy', e=None), dict(op='hit', e=None, v=0, mod=ONE),
                                                dict(op='defence', v=None, a=0), dict(op='defence', v=0, a=None),
                                                dict(op='hit', e=0, v=None, mod=ONE)])

    # Integer wrapping and degenerate divisions in the hit chance.
    extremes = [2147483647, -2147483648, 2147483638, -9, -8, 46340]
    for level in extremes:
        for bonus in [-65, -64, -63, 2147483647, 2147483583, -2147483648]:
            for kind in ['melee', 'range', 'magic']:
                attacker = npc(levels=levels(attack=level, strength=level, defence=level, range=level, magic=level), bonuses=[bonus] * 15)
                victim = npc(levels=levels(attack=-8, strength=1, defence=-8, range=1, magic=-8), bonuses=[-65] * 15)
                if (level + bonus) % 2:
                    add(kind, [attacker, victim], calc(kind))
                else:
                    add(kind, [dict(attacker, kind='player', attackStyle=[1, 2]), dict(victim, kind='player')], calc(kind))

    # Magic base damage, level modifiers and the under-two formula.
    damage = [0.0, 1.0, 1.5, 1.99, 2.0, 2.01, 3.0, 10.0, 20.0, 30.5, float('nan'), float('inf'), -5.0, 1e12, -0.0]
    for level, bonus in [(1, 0), (50, 10), (99, 20), (99, -40), (120, 150), (0, -1)]:
        for extra in [0.0, 0.1, -0.5, 1.0, float('nan'), -0.0, 4.0e9]:
            if extra not in (0.0, 0.1) and level != 99:
                continue
            for who in ['player', 'npc', 'other']:
                caster = dict(kind=who, levels=levels(magic=level), bonuses=bonuses({13: bonus, 14: bonus * 2 + 1}), levelMod=H(extra))
                add('magic', [caster, npc()], [dict(op='hit', e=0, v=1 if index % 2 else None, mod=H(mod)) for index, mod in enumerate(damage)])

    # The accuracy decision: overloads, modifiers, handler type and exact boundary draws.
    mods = [1.0, 1.1, 0.75, 0.0, -1.0, float('nan')]
    for kind in ['melee', 'range', 'magic']:
        style = STYLE_OF[kind]
        pairs = [[dict(base, kind='player'), dict(target, kind='npc')], [dict(base, kind='npc'), dict(target, kind='player')],
                 [dict(base, kind='player'), dict(target, kind='player')],
                 [dict(base, kind='familiar'), dict(target, kind='player', prayers=['PROTECT_FROM_SUMMONING'])],
                 [dict(base, kind='familiar'), dict(target, kind='player')],
                 [dict(base, kind='familiar', levels=levels(attack=-20, range=-20, magic=-20)),
                  dict(target, kind='player', prayers=['PROTECT_FROM_SUMMONING'])],
                 [dict(base, kind='npc', levels=levels(attack=1, range=1, magic=1), bonuses=[-64] * 15), dict(target, kind='npc')],
                 [dict(base, kind='npc', levels=levels(attack=1, range=1, magic=1), bonuses=[-70] * 15), dict(target, kind='npc')]]
        for entities in pairs:
            ops = [dict(op='boundary', e=0, v=1, style=style, accMod=H(a), defMod=H(d)) for a in mods for d in mods]
            ops += [dict(op='boundary', e=1, v=0, style=other, accMod=H(float('inf')), defMod=H(2.0)) for other in ['MELEE', 'RANGE', 'MAGIC']]
            add(kind, entities, ops)
            for seed in range(6):
                ops = [dict(op='accurate', e=0, v=1), dict(op='accurate', e=0, v=1, style=style), dict(op='accurate', e=0, v=1, style=None),
                       dict(op='accurate', e=0, v=None), dict(op='accurate', e=None, v=1, style=style),
                       dict(op='accurate', e=0, v=1, style='MAGIC', accMod=H(1.1), defMod=H(0.9)),
                       dict(op='accurate', e=1, v=0, style='MELEE', accMod=ONE, defMod=H(0.0)),
                       dict(op='setType', style=None), dict(op='accurate', e=0, v=1), dict(op='threshold', e=0, v=1, style=None, accMod=ONE, defMod=ONE),
                       dict(op='setType', style='RANGE'), dict(op='accurate', e=0, v=1), dict(op='accurate', e=1, v=0, style=style)]
                add(kind, entities, ops, rng=dict(mathSeed=seed * 7919 + 1, intSeed=seed))

    # Melee swings.
    weapon = {'3': [4151, 1, 1000, 'Abyssal whip']}
    fighters = dict(
        pvn=[dict(base, kind='player', equipment=weapon), dict(target, kind='npc', lifepoints=60)],
        nvp=[dict(base, kind='npc'), dict(target, kind='player', lifepoints=60)],
        pvp=[dict(base, kind='player'), dict(target, kind='player', lifepoints=60, prayerBonus=prayer_bonus([boosting[-1]]))],
        fvp=[dict(base, kind='familiar'), dict(target, kind='player', lifepoints=60, prayers=['PROTECT_FROM_SUMMONING'])],
        ovn=[dict(base, kind='other'), dict(target, kind='npc', lifepoints=60)],
        verac=[dict(base, kind='player', armourSet='VERAC', equipment=weapon), dict(target, kind='player', lifepoints=60, protection=['MELEE'])],
        veracnpc=[dict(base, kind='npc', armourSet='VERAC'), dict(target, kind='player', lifepoints=60, protection=['MELEE'])],
        veracfree=[dict(base, kind='npc', armourSet='VERAC'), dict(target, kind='player', lifepoints=60)],
        dharok=[dict(base, kind='player', armourSet='DHAROK', lifepoints=1, maxLifepoints=99), dict(target, kind='npc', lifepoints=60)],
        weak=[dict(base, kind='player', levels=levels(attack=1, strength=1), bonuses=bonuses({11: -64})), dict(target, kind='npc', lifepoints=60)],
        negative=[dict(base, kind='player', levels=levels(attack=1, strength=1), bonuses=bonuses({11: -3000})), dict(target, kind='npc', lifepoints=60)],
    )
    never, always = H(0.9999999999999999), H(0.0)
    for label, entities in fighters.items():
        for seed in range(4):
            add('melee', entities, [dict(op='swing', e=0, v=1, state=dict(attacker=0, victim=1))] * 3,
                rng=dict(mathSeed=seed * 104729 + 3, intSeed=seed * 31 + 5),
                sweep=seed == 0 and label in ('pvn', 'nvp', 'fvp', 'verac', 'veracnpc', 'dharok'))
        states = [dict(attacker=0, victim=1), dict(attacker=0, victim=1, secondaryHit=3), dict(ctor=False, secondaryHit=50),
                  dict(attacker=1, victim=0, armourEffect='VERAC'), dict(attacker=0, victim=1, estimatedHit=44, maximumHit=9, armourEffect='DHAROK')]
        draws = [([always], ['max']), ([always], [0]), ([never], []), ([always], [0, 'max']), ([always], [1, 'max']),
                 ([always], [1, 0]), ([never], [1, 'max']), ([never], [3]), ([always], [2, 'max'])]
        for index, (math, ints) in enumerate(draws):
            for turn, life in enumerate([60, 5, 0]):
                for state in [states[(index + turn) % 5]] + ([states[3]] if label.startswith('verac') and life == 5 else []):
                    pair = [entities[0], dict(entities[1], lifepoints=life)]
                    add('melee', pair, [dict(op='swing', e=0, v=1, state=state)], rng=dict(math=math * 2, ints=ints + ['max'] * 3),
                        sweep=life == 5 and 'armourEffect' in state and ints == [1, 'max'] and label in ('verac', 'veracnpc', 'pvp'))
        add('melee', entities, [dict(op='swing', e=0, v=None, state=dict(attacker=0)), dict(op='swing', e=None, v=1, state=dict(victim=1)),
                                dict(op='swing', e=None, v=None, state=dict(ctor=False))], rng=dict(mathSeed=9, intSeed=9))

    # Range data, calculations and swings.
    weapons = [dict(itemId=841, slot=13, type=0, ammunition=[882, 884, 886]), dict(itemId=9185, slot=13, type=1, ammunition=[9140, 9243, 9244, 877]),
               dict(itemId=11235, slot=13, type=2, ammunition=[882, 892, 11212]), dict(itemId=868, slot=3, type=3, ammunition=[868]),
               dict(itemId=4214, slot=13, type=4, ammunition=[]), dict(itemId=4675, slot=13, type=5, ammunition=[882]),
               dict(itemId=10033, slot=3, type=6, ammunition=[10033]), dict(itemId=7000, slot=13, type=7, ammunition=[882]),
               dict(itemId=7001, slot=14, type=0, ammunition=[882]), dict(itemId=7002, slot=-1, type=0, ammunition=[882]),
               dict(itemId=7003, slot=5, type=0, ammunition=[1171]), dict(itemId=7004, slot=13, type=-1, ammunition=[882])]
    ammo = [dict(itemId=i, poisonDamage=p) for i, p in [(882, 0), (892, 0), (11212, 0), (9140, 0), (9243, 0), (9244, 0), (868, 10), (10033, 0), (0, 0)]]
    table = dict(weapons=weapons, ammo=ammo)

    def table_for(equipment):
        """Only the rows a loadout can look up: its weapon, its worn items and the id-0 placeholder."""
        worn = {row[0] for row in equipment.values()} | {0}
        held = equipment.get('3', [None])[0]
        return dict(weapons=[w for w in weapons if w['itemId'] == held], ammo=[a for a in ammo if a['itemId'] in worn])

    archer =dict(base, kind='player', attackStyle=[4, 4], location=[3200, 3200, 0], levels=levels(range=90, defence=75, slayer=50))

    def gear(weapon_id, name, ammo_id=None, amount=100, slot=13, arrows=None):
        out = {'3': [weapon_id, 1, 1000, name]}
        if ammo_id is not None:
            out[str(slot)] = [ammo_id, amount, 1000, 'Ammo', arrows] if arrows else [ammo_id, amount, 1000, 'Ammo']
        return out

    loadouts = dict(
        bow=gear(841, 'Shortbow', 882), bowwrong=gear(841, 'Shortbow', 892), bowone=gear(841, 'Shortbow', 882, 1), bowzero=gear(841, 'Shortbow', 882, 0),
        bownone=gear(841, 'Shortbow'), crossbow=gear(9185, 'Rune crossbow', 9140), diamond=gear(9185, 'Rune crossbow', 9243),
        dragon=gear(9185, 'Rune crossbow', 9244), darkbow=gear(11235, 'Dark bow', 892, 50), darkone=gear(11235, 'Dark bow', 892, 1),
        darknone=gear(11235, 'Dark bow'), darkwrong=gear(11235, 'Dark bow', 9140, 2), knife=gear(868, 'Rune knife', None),
        knives=dict(gear(868, 'Rune knife'), **{'3': [868, 40, 1000, 'Rune knife'], '13': [882, 5, 1000, 'Bronze arrow', [0] * 14 + [7]]}),
        knivesnull=dict({'3': [868, 40, 1000, 'Rune knife'], '13': [882, 5, 1000, 'Bronze arrow']}),
        crystal=gear(4214, 'Crystal bow'), staff=gear(4675, 'Ancient staff', 882), chin={'3': [10033, 9, 1000, 'Chinchompa']},
        badtype=gear(7000, 'Type seven', 882), badslot=gear(7001, 'Slot fourteen', 882), noslot=gear(7002, 'Slot minus one', 882),
        shield=dict(gear(7003, 'Shield ammo'), **{'5': [1171, 3, 1000, 'Wooden shield']}), negative=gear(7004, 'Type minus one', 882),
        unknown=gear(4151, 'Abyssal whip', 882), unarmed={'13': [882, 10, 1000, 'Bronze arrow']}, empty={},
    )
    swept = ('bow', 'bowwrong', 'bownone', 'darkone', 'knives', 'unknown')
    for label, equipment in loadouts.items():
        shooter = dict(archer, equipment=equipment)
        rows_for = table_for(equipment)
        for who, task in [('npc', None), ('npc', dict(levelReq=50)), ('npc', dict(levelReq=51)), ('player', None)]:
            victim = dict(target, kind=who, lifepoints=60, location=[3203, 3204, 0], npcTask=task)
            if task and label not in ('diamond', 'dragon', 'bow'):
                continue
            add('range', [shooter, victim], calc('range')[:4], range=rows_for)
            for seed in range(2):
                add('range', [shooter, victim], [dict(op='swing', e=0, v=1, state=dict(attacker=0, victim=1))] * 2, range=rows_for,
                    rng=dict(mathSeed=seed * 15485863 + 11, intSeed=seed * 97 + 2), sweep=seed == 0 and task is None and who == 'npc' and label in swept)
            states = [dict(attacker=0, victim=1), dict(attacker=None, victim=1), dict(attacker=0, victim=None),
                      dict(attacker=1, victim=0), dict(ctor=False, secondaryHit=40, estimatedHit=9)]
            draws = [([always, always], [5, 'max', 'max']), ([always, never], [5, 'max']), ([never, always], [4, 'max']),
                     ([never, never], [5, 0]), ([always, always], [0, 0, 0]), ([never, never], [])]
            for index, (math, ints) in enumerate(draws):
                if who == 'player' and index in (2, 4, 5) and label not in ('diamond', 'darkbow'):
                    continue
                state = states[0] if index == 0 or (index + len(label)) % 2 else states[1 + (index + len(label)) % 4]
                add('range', [shooter, dict(victim, lifepoints=7)], [dict(op='swing', e=0, v=1, state=state)], range=rows_for,
                    rng=dict(math=math + [never], ints=ints + [0] * 4),
                    sweep=label in ('diamond', 'darkbow') and who == 'player' and index == 0)
            if label in ('diamond', 'darkbow', 'bow'):
                for state in states[1:]:
                    add('range', [shooter, dict(victim, lifepoints=7)], [dict(op='swing', e=0, v=1, state=state)], range=rows_for,
                        rng=dict(math=[always, always, never], ints=[5, 'max', 'max', 0]))
    for flags in [(), ('IGNORE_STAT_REDUCTION',), ('IGNORE_STAT_REDUCTION', 'IGNORE_STAT_REDUCTION'), tuple(FLAGS)]:
        for static in [1, 60, 99]:
            shooter = dict(archer, equipment=loadouts['diamond'], levels=levels(range=60, slayer=1), staticLevels=levels(range=static),
                           familiar=12786, void=['RANGE'], perks=['ACCURATE_MARKSMAN'])
            victim = dict(target, kind='npc', lifepoints=99, location=[3210, 3200, 0])
            swing = [dict(op='swing', e=0, v=1, state=dict(attacker=0, victim=1))]
            add('range', [shooter, victim], calc('range')[:4] + swing + calc('range')[1:2], range=table_for(loadouts['diamond']),
                flags=flags, rng=dict(math=[always] * 4, ints=[5, 'max', 'max', 'max']))
            add('range', [shooter, victim], swing, range=table_for(loadouts['diamond']), flags=flags,
                rng=dict(math=[always] * 4, ints=[5, 'max', 'max', 'max']), sweep=static == 99 and len(flags) < 2)
    for dx, dy in [(0, 0), (1, 0), (2, 2), (3, 4), (0, 7), (10, 0), (7, 7), (-9, 3), (40, 30), (46341, 0), (2147480000, 1)]:
        for who in ['player', 'npc']:
            shooter = dict(archer, kind=who, equipment=loadouts['bow'])
            for place in [[3200 + dx, 3200 + dy, 0], [3200 + dx, 3200 + dy, 2], None]:
                victim = dict(target, kind='npc', lifepoints=60, location=place)
                add('range', [shooter, victim], [dict(op='swing', e=0, v=1, state=dict(attacker=0, victim=1))], range=table_for(loadouts['bow']),
                    rng=dict(mathSeed=dx, intSeed=dy))
    add('range', [dict(archer, equipment=loadouts['bow'], location=None), dict(target, kind='npc')],
        [dict(op='swing', e=0, v=1, state=dict(attacker=0, victim=1))], range=table_for(loadouts['bow']))
    add('range', [dict(archer, equipment=loadouts['bow']), dict(target, kind='npc')],
        [dict(op='swing', e=None, v=1, state=dict(victim=1)), dict(op='swing', e=0, v=None, state=dict(attacker=0)),
         dict(op='swing', e=None, v=None, state=dict(ctor=False)), dict(op='swing', e=1, v=None, state=dict(attacker=1))], range=table,
        rng=dict(mathSeed=4, intSeed=4))

    # Magic swings.
    def book(*rows):
        return [dict(row) for row in rows]

    wizard = dict(base, kind='player', location=[3200, 3200, 0], levels=levels(magic=88, defence=75), attackStyle=[8, 3])
    spells = book(dict(maxImpact=20, type='WAVE'), dict(maxImpact=26, type='BLITZ'), dict(maxImpact=1, type='STRIKE'),
                  dict(maxImpact=0, type=None), dict(maxImpact=30, type='BARRAGE', targets=[[1], [2], [1]]),
                  dict(maxImpact=8, type='BURST', targets=[[1], None, [2]]), dict(maxImpact=8, type='RUSH', targets=None),
                  dict(maxImpact=8, type='RUSH', targets=[]), dict(maxImpact=20, type='BOLT', meets=False), dict(maxImpact=2, type='BIND'),
                  dict(maxImpact=3, type='SNARE'), dict(maxImpact=-4, type='CURSE'))
    for who in ['player', 'npc', 'other']:
        for chosen, auto in itertools.product([None] + list(range(len(spells))), [None, 0, 1, 8]):
            if chosen not in (None, 0, 1, 8) and auto not in (None, 0):
                continue
            for interface in [None, 'STAFF']:
                if interface and chosen != 8 and auto != 8:
                    continue
                # Only the spells this caster holds travel with the case, in first-use order.
                used = [i for i in dict.fromkeys([chosen, auto]) if i is not None]
                held = [spells[i] for i in used]
                caster = dict(wizard, kind=who, spell=None if chosen is None else used.index(chosen),
                              autocast=None if auto is None else used.index(auto), weaponInterface=interface,
                              levelMod=H(0.1 if chosen == 2 else 0.0))
                victims = [dict(target, kind='npc', lifepoints=12, location=[3203, 3204, 0]),
                           dict(target, kind='player', lifepoints=60, location=[3200, 3209, 0], prayerBonus=prayer_bonus([boosting[4]]))]
                for seed in range(2 if who == 'player' else 1):
                    add('magic', [caster] + victims, [dict(op='swing', e=0, v=1, state=dict(attacker=0, victim=1))] * 2, spells=held,
                        rng=dict(mathSeed=seed * 6700417 + 13, intSeed=seed * 211 + 1),
                        sweep=seed == 0 and who == 'player' and auto in (None, 8) and chosen in (None, 0, 4, 5, 8))
                if who != 'player' and chosen not in (None, 0, 4):
                    continue
                draws = [([always] * 3, ['max'] * 3), ([always] * 3, [0] * 3), ([never] * 3, []), ([always, never, always], ['max', 0])]
                for index, (math, ints) in enumerate(draws):
                    state = [dict(attacker=0, victim=1), dict(ctor=False, estimatedHit=30, secondaryHit=5)][index % 2]
                    add('magic', [caster] + victims, [dict(op='swing', e=0, v=1, state=state)], spells=held,
                        rng=dict(math=math, ints=ints + [0]))
    for dx, dy in [(0, 0), (1, 0), (2, 2), (3, 4), (0, 7), (10, 0), (-9, 3), (46341, 0)]:
        for chosen in [0, 1]:
            for place in [[3200 + dx, 3200 + dy, 0], None]:
                add('magic', [dict(wizard, spell=chosen), dict(target, kind='npc', lifepoints=60, location=place)],
                    [dict(op='swing', e=0, v=1, state=dict(attacker=0, victim=1))], spells=spells[:2], rng=dict(mathSeed=dx + 1, intSeed=dy))
    add('magic', [dict(wizard, spell=0), dict(target, kind='npc'), dict(wizard, spell=0, location=None)],
        [dict(op='swing', e=None, v=1, state=dict(victim=1)), dict(op='swing', e=0, v=None, state=dict(attacker=0)),
         dict(op='swing', e=2, v=1, state=dict(attacker=2, victim=1)), dict(op='swing', e=1, v=0, state=dict(attacker=1))], spells=spells,
        rng=dict(mathSeed=4, intSeed=4))

    # Random mixtures of every input above.
    rng = random.Random(53002)
    pool_levels = [0, 1, 1, 10, 40, 60, 70, 75, 80, 90, 99, 99, 104, 118, 125]
    pouches = [None, None, None, 12822, 12790, 12089, 12786, 7, 'throw', 'manager']
    all_styles = sorted({s for _, styles in interfaces for s in styles})

    def mixed(kind_name):
        active = rng.sample(boosting, rng.choice([0, 0, 1, 1, 2, 3]))
        style = rng.choice(all_styles)
        return dict(kind=kind_name, levels={str(s): rng.choice(pool_levels) for s in [0, 1, 2, 4, 6, 18]},
                    staticLevels={'4': rng.choice(pool_levels)}, bonuses=[rng.randrange(-90, 260) for _ in range(15)],
                    attackStyle=list(style), prayerBonus=prayer_bonus(active), armourSet=rng.choice([None, None] + SETS),
                    void=rng.sample(['MELEE', 'RANGE', 'MAGIC'], rng.choice([0, 0, 1, 3])),
                    perks=rng.sample(['PRECISION_STRIKES', 'ACCURATE_MARKSMAN'], rng.choice([0, 0, 1, 2])), familiar=rng.choice(pouches),
                    lifepoints=rng.choice([0, 1, 9, 50, 99, 150]), maxLifepoints=rng.choice([10, 99, 110]), pulseVictim=rng.choice([None, 1, 1]),
                    name=rng.choice(UNDEAD), id=rng.choice([0, 55, 1158, 2030]), slayerBonus=H(rng.choice([1.0, 1.075, 1.1667])),
                    slayerTask=rng.choice([None, dict(ids=[55]), dict(ids=[2], kalphites=True), dict(ids=[2030, 55], dragon=True)]),
                    inEquipment=rng.choice([[], ['12862']]), prayers=rng.choice([[], ['PROTECT_FROM_SUMMONING']]),
                    protection=rng.sample(['MELEE', 'RANGE', 'MAGIC'], rng.choice([0, 1])), levelMod=H(rng.choice([0.0, 0.0, 0.1, 0.25])),
                    equipment=rng.choice([{}, {'2': [4081, 1, 1000, 'Salve amulet']}, {'2': [10588, 1, 1000, 'Salve amulet (e)']},
                                          {'3': [841, 1, 1000, 'Shortbow'], '13': [882, 60, 1000, 'Bronze arrow']},
                                          {'3': [868, 3, 1000, 'Rune knife'], '13': [882, 60, 1000, 'Bronze arrow', [0] * 14 + [rng.randrange(-5, 40)]]},
                                          {'3': [4151, 1, 1000, 'Abyssal whip']}]),
                    location=[3200 + rng.randrange(-6, 7), 3200 + rng.randrange(-6, 7), 0])

    for index in range(210):
        kind = ['melee', 'range', 'magic'][index % 3]
        entities = [mixed(rng.choice(['player', 'player', 'player', 'npc', 'familiar'])), mixed(rng.choice(['player', 'npc', 'npc']))]
        flags = rng.sample(FLAGS, rng.choice([0, 0, 0, 1, 2]))
        mod = H(rng.choice([1.0, 1.0, 0.5, 1.1, 1.2, 2.0, 0.0, 20.0, 16.5]))
        ops = calc(kind, mod=mod)[:4] + [dict(op='boundary', e=0, v=1, style=STYLE_OF[kind], accMod=ONE, defMod=ONE)]
        extra = dict(rng=dict(mathSeed=rng.randrange(1 << 40), intSeed=rng.randrange(1 << 40)))
        if kind == 'magic':
            entities[0]['spell'] = rng.choice([None, 0, 1, 2])
            entities[0]['autocast'] = rng.choice([None, 0, 1])
            extra['spells'] = book(dict(maxImpact=rng.choice([0, 1, 2, 3, 16, 20, 30]), type='WAVE'), dict(maxImpact=24, type='BLITZ'),
                                   dict(maxImpact=11, type='BURST', targets=[[1], [0]]))
        if kind == 'range':
            extra['range'] = table_for(entities[0]['equipment'])
        ops += [dict(op='swing', e=0, v=1, state=dict(attacker=0, victim=1))] * 2
        add(kind, entities, ops, flags=flags, **extra)

    # Remaining pure helpers of the base class and BattleState.
    for kind in ['melee', 'range', 'magic']:
        ops = []
        for size_a, size_v, distance in itertools.product([1, 2, 3, 5], [1, 2, 4], [0, -1, 3]):
            ops = [dict(op='combatDistance', e=0, v=1, raw=raw) for raw in [1, 7, 10, -3, 2147483647]]
            ops += [dict(op='combatDistance', e=1, v=0, raw=raw) for raw in [1, 7, 10, -3]]
            add(kind, [player(size=size_a), npc(size=size_v, combatDistance=distance)], ops)
        add(kind, [player(), dict(kind='familiar', size=3, combatDistance=6), dict(kind='other', size=2)],
            [dict(op='combatDistance', e=1, v=0, raw=4), dict(op='combatDistance', e=2, v=1, raw=4),
             dict(op='combatDistance', e=None, v=1, raw=4), dict(op='combatDistance', e=0, v=None, raw=4)])
        for life in [0, 1, 5, 10, 99]:
            add(kind, [npc(lifepoints=life)], [dict(op='formatHit', v=0, raw=raw) for raw in [-5, 0, 1, 4, 5, 6, 10, 100, 2147483647, -2147483648]]
                + [dict(op='formatHit', v=None, raw=3)])
        add(kind, [], [dict(op='dragonfire', protection=p, name=n) for p in list(range(16)) + [-1, 64, 0x7ffffff4] for n in ['fiery breath', 'icy breath', '']])
        holdings = [[], ['4716', '4718', '4720', '4722'], ['4880', '4889', '4892', '4901'], ['4716', '4718', '4720'], ['4724', '4726', '4728', '4730'],
                    ['4753', '4755', '4757', '4759'], ['11128', '6523'], ['11128', '6528'], ['11128'], ['6525'], ['4745', '4747', '4749', '4751'],
                    ['4732', '4734', '4736', '4738'], ['4708', '4710', '4712', '4714'], ['4856', '4865', '4871', '4877'],
                    ['4716', '4718', '4720', '4722', '4724', '4726', '4728', '4730', '11128', '6527'], ['4753', '4755', '4757', '4759', '4745', '4747', '4749', '4751']]
        for held in holdings:
            add(kind, [player(contains=held)], [dict(op='armourSet', e=0)])
        for npc_id in [2024, 2025, 2026, 2027, 2028, 2029, 2030, 2031]:
            add(kind, [npc(id=npc_id), dict(kind='familiar', id=npc_id), dict(kind='other', id=npc_id)],
                [dict(op='armourSet', e=0), dict(op='armourSet', e=1), dict(op='armourSet', e=2), dict(op='armourSet', e=None)])
    hits = [-2147483648, -1, 0, 7, 2147483647]
    for first in hits:
        ops = []
        for second in hits:
            child = dict(estimatedHit=second, secondaryHit=first)
            for targets in [None, [], ['this'], [None], [child], ['this', child, None, dict(child, targets=[child])], [child, child, dict(ctor=False)]]:
                state = dict(attacker=0, victim=1, estimatedHit=first, secondaryHit=second)
                if targets is not None:
                    state['targets'] = targets
                ops += [dict(op='total', state=state), dict(op='neutralize', state=state)]
        add('melee', [player(), npc()], ops)
    add('melee', [], [dict(op='total', state=dict(ctor=False)), dict(op='neutralize', state=dict(ctor=False))])
    return rows


def substitute(text, values):
    for key, value in values.items():
        marker = '/*' + key + '*/'
        assert marker in text, key
        text = text.replace(marker, value)
    return text


def constants_jar(read, run, lock):
    """The pinned bundled constants jar, taken from the reference object store."""
    evidence = json.loads(read(ROOT / 'oracle/pickup/consts-evidence.json'))
    blob = run(['git', '-C', ROOT / 'reference/server', 'show', lock['server']['commit'] + ':Server/libs/ConstLib-1.4.jar']).stdout
    assert len(blob) == evidence['bytes'] and hashlib.sha256(blob).hexdigest() == evidence['sha256']
    jar = RUN / 'ConstLib-1.4.jar'
    if not jar.is_file() or jar.read_bytes() != blob:
        jar.write_bytes(blob)
    return jar, evidence['sha256']


def build():
    read, run = c.read, c.run
    lock = json.loads(read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        repo = ROOT / 'reference' / part
        assert run(['git', '-C', repo, 'rev-parse', 'HEAD']).stdout.decode().strip() == lock[part]['commit']
        run(['git', '-C', repo, 'diff', '--quiet', 'HEAD', '--'])
    classes = RUN / 'classes'
    classes.mkdir(parents=True, exist_ok=True)
    for stale in classes.rglob('*.class'):
        stale.unlink()
    jars = c.artifacts()
    std = next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    artifact = json.loads(read(ROOT / 'oracle/auth/json.lock.json'))
    simple = RUN / 'json-simple-1.1.1.jar'
    if not simple.exists() or hashlib.sha256(simple.read_bytes()).hexdigest() != artifact['sha256']:
        with urllib.request.urlopen(artifact['url'], timeout=60) as response:
            data = response.read()
        assert len(data) == artifact['size'] and hashlib.sha256(data).hexdigest() == artifact['sha256']
        simple.write_bytes(data)
    consts, consts_hash = constants_jar(read, run, lock)

    def source(name):
        return read(SOURCE / name)

    skills = source('core/game/node/entity/skill/Skills.java')
    weapon = source(COMBAT + 'equipment/WeaponInterface.java')
    pouch = source('content/global/skill/summoning/SummoningPouch.java')
    assert 'private SummoningPouch(int slot, int pouchId,' in pouch
    pouches = []
    for name in ['WOLPERTINGER_POUCH', 'GEYSER_TITAN_POUCH', 'IRON_TITAN_POUCH', 'STEEL_TITAN_POUCH']:
        pouches.append(name + '(' + re.search(r'\b' + name + r'\(\d+, (?:(?:true|false), )?(\d+),', pouch).group(1) + ')')
    graphics = source('core/game/world/update/flag/context/Graphics.java')
    body = graphics[graphics.index('{', graphics.index('public class Graphics')) + 1:graphics.rindex('}')]
    body = body.replace(c.method(graphics, 'public static void send(Graphics graphic, Location l)'), '')
    config = source('core/game/system/config/ItemConfigParser.kt')
    pathfinder = source('core/game/world/map/path/Pathfinder.java')
    prayer = source('core/game/node/entity/player/link/prayer/PrayerType.java')
    spell = source(COMBAT + 'spell/SpellType.java')
    values = dict(
        SKILL_CONSTANTS='\n'.join(re.findall(r'^\s*public static final int (?:ATTACK|NUM_SKILLS) = .*;$', skills, re.M)),
        POUCH_CONSTANTS=','.join(pouches),
        SLOT_CONSTANTS=re.search(r'public static final int SLOT_HAT = .*;', source('core/game/container/impl/EquipmentContainer.java')).group(),
        ITEM_CONFIG_CONSTANTS='\n'.join(f' public static final String {n} = "{v}";' for n, v in re.findall(r'const val (\w+) = "(.*)"', config)),
        PRAYER_CONSTANTS=','.join(re.findall(r'^\t([A-Z_]+)\(', prayer, re.M)),
        SPELL_TYPE_CONSTANTS=','.join(re.findall(r'^\t([A-Z_]+)\(', spell, re.M)),
        WEAPON_INTERFACE_CONSTANTS='\n'.join(re.findall(r'^\s*public static final int (?:BONUS|STYLE)_\w+ = \d+;$', weapon, re.M)),
        WEAPON_INTERFACE_TYPES=c.method(weapon, 'public static class AttackStyle') + '\n' + c.method(weapon, 'public static enum WeaponInterfaces'),
        GRAPHICS_BODY=body,
        PREVENT_CONSTANTS='\n'.join(re.findall(r'^\s*public static final int PREVENT_\w+ = \w+;$', pathfinder, re.M)),
    )
    java, kotlin = [], []

    def adapter(name, content):
        path = RUN / 'adapters' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding='utf-8', newline='\n')
        (java if path.suffix == '.java' else kotlin).append(path)

    stale = RUN / 'adapters'
    if stale.is_dir():
        for path in sorted(stale.rglob('*'), reverse=True):
            path.unlink() if path.is_file() else path.rmdir()
    text = substitute(read(HERE / 'adapters.txt'), values)
    parts = re.split(r'^//// (\S+)\n', text, flags=re.M)
    for name, content in zip(parts[1::2], parts[2::2]):
        adapter(name, content)
    location = source('core/game/world/map/Location.kt')
    adapter('core/game/world/map/Location.kt', '''package core.game.world.map
import kotlin.math.sqrt
// Coordinates, the original distance arithmetic and the 3-argument transform; other helpers are not driven.
class Location(val x: Int, val y: Int, val z: Int) {
    companion object {
''' + c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location)') + '''
    }
''' + '\n'.join(line for line in location.splitlines() if 'fun getDistance(other: Location)' in line or 'fun transform(diffX: Int, diffY: Int, z: Int)' in line) + '''
    fun transform(dir: Direction): Location = throw oracle.Support.unsupported("Location.transform(Direction)")
    fun withinDistance(other: Location, dist: Int): Boolean = throw oracle.Support.unsupported("Location.withinDistance")
    fun deriveDirection(location: Location): Direction? = throw oracle.Support.unsupported("Location.deriveDirection")
    fun getStepComponents(dir: Direction): List<Location> = throw oracle.Support.unsupported("Location.getStepComponents")
    override fun equals(other: Any?): Boolean = other is Location && other.x == x && other.y == y && other.z == z
    override fun hashCode(): Int = z shl 30 or (x shl 15) or y
}
''')
    randoms = source('core/tools/RandomFunction.java')
    chosen = ['public static final int random(int a, int b)', 'public static boolean roll(int chance)',
              'public static final int random(int maxValue)', 'public static int randomize(int value)']
    adapter('core/tools/RandomFunction.java', 'package core.tools;\n// Selected original methods, verbatim; RANDOM is assignable so a case can bind its draws.\npublic class RandomFunction{public static java.util.Random RANDOM;\n'
            + '\n'.join(c.method(randoms, s) for s in chosen) + '}\n')
    for name in UNMODIFIED:
        path = SOURCE / name
        read(path)
        (java if path.suffix == '.java' else kotlin).append(path)
    read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    deps = os.pathsep.join(map(str, [std, simple, consts]))
    cp = os.pathsep.join(map(str, [classes, std, simple, consts]))
    run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
         '-jvm-target', '11', '-nowarn', '-classpath', deps, '-d', classes, *kotlin, *java])
    run(['javac', '--release', '11', '-nowarn', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    return lock, cp, consts_hash


def execute(cp, cases):
    exe = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    process = subprocess.run([str(exe), '-Xmx1g', '-Dfile.encoding=UTF-8', '-cp', cp, 'oracle.Support'], input=payload,
                             capture_output=True, cwd=ROOT, timeout=900)
    if process.returncode:
        raise RuntimeError(process.stderr.decode(errors='replace'))
    results = [json.loads(line) for line in process.stdout.splitlines() if line]
    assert len(results) == len(cases), (len(results), len(cases))
    return results


def generate(check):
    lock, cp, consts_hash = build()
    interfaces, prayers = tables()
    planned = corpus(interfaces, prayers)
    cases = [case for case, _ in planned]
    results = execute(cp, cases)
    # Second pass: every service call and draw of the marked single swings fails once.
    failing = []
    for (case, sweep), result in zip(planned, results):
        if not sweep:
            continue
        for at in range(1, len(result[0]['events']) + 1):
            failing.append(dict(case, ops=[dict(case['ops'][0], failAt=at)]))
    cases += failing
    results += execute(cp, failing)
    for case, result in zip(cases, results):
        for op, row in zip(case['ops'], result):
            error = row['error']
            assert error in (None, 'Null', 'Index', 'Binding'), (error, op, case['handler'])
            assert error != 'Binding' or 'failAt' in op, (op, case)
    data = ''.join(json.dumps(dict(input=case, expected=result), sort_keys=True, separators=(',', ':')) + '\n'
                   for case, result in zip(cases, results)).encode()
    c.read(Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations = sum(len(case['ops']) for case in cases)
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(cases), operationCount=operations,
        failureCaseCount=len(failing), fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm='sha256, CR removed',
        kotlinCompiler='1.8.20', constantsJar=dict(source='reference/server git ' + lock['server']['commit'] + ':Server/libs/ConstLib-1.4.jar',
                                                   sha256=consts_hash, missingNamedConstant='Sounds.HUMAN_ATTACK_2564',
                                                   numericAdapter={'HUMAN_ATTACK_2564': 2564}),
        unmodified=UNMODIFIED,
        scope='Unmodified CombatSwingHandler, MeleeSwingHandler, RangeSwingHandler, MagicSwingHandler, BattleState, CombatStyle, '
              'InteractionType, Weapon, RangeWeapon, Ammunition, ArmourSet, BoltEffect and EquipmentSlot. Verbatim selected '
              'RandomFunction methods, Location distance/transform, Graphics constructors, and the AttackStyle/WeaponInterfaces '
              'declarations and style/bonus constants of WeaponInterface. Entity, Player, NPC, Familiar, Skills, Properties, prayer, '
              'perk, slayer, familiar, equipment, item, spell, audio/graphics/message/log and combat-pulse members are recording '
              'stand-ins answering from the case input; stand-ins for paths this corpus does not drive throw. Math.random is bound '
              'to the case generator by replacing java.lang.Math private generator once at start. Drives calculateAccuracy/'
              'calculateHit/calculateDefence/getSetMultiplier/getArmourSet/isAccurateImpact, swing, getCombatDistance, formatHit, '
              'getDragonfireMessage and BattleState totals for the three handlers. Does not drive canSwing/isAttackable/'
              'canStepTowards/canMelee, adjustBattleState, getFormattedHit, addExperience, impact, visualize, onImpact, useAmmo, '
              'special-attack registration, BoltEffect or ArmourSet effects other than the DIAMOND path inside RangeSwingHandler.swing, '
              'RangedConfigLoader, or any original class behind a stand-in.')
    folder = ROOT / 'fixtures/combat-swing'
    if check:
        assert (folder / 'oracle.jsonl').read_bytes() == data, 'combat-swing fixture differs'
        assert json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) == provenance, 'combat-swing provenance differs'
        print(f'{len(cases)} original combat swing cases ({operations} operations) reproduced.')
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'oracle.jsonl').write_bytes(data)
        (folder / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {len(cases)} original combat swing cases ({operations} operations).')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
