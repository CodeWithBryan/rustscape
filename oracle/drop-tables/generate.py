"""Run the unchanged NPC drop-table classes on JDK 11 and record what they hold and roll.

Compiled unchanged: WeightBasedTable.kt, WeightedItem.kt, NPCDropTable.kt,
Exceptions.kt, DropTableParser.kt, RareDropTable.kt, the six shared
*DropTable.java tables, NPCDropTables.java, DropFrequency.java,
RandomFunction.java, WeightedChanceItem.java, ChanceItem.java and
StartupListener/ContentInterface. Everything else they name is an adapter under
oracle/drop-tables/adapters. --check never updates a fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import struct
import subprocess
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/drop-tables-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/drop-tables'
spec = importlib.util.spec_from_file_location('compiler', ROOT / 'oracle/login/generate.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)

ORIGINAL = [
    'core/api/utils/WeightBasedTable.kt',
    'core/api/utils/WeightedItem.kt',
    'core/api/utils/NPCDropTable.kt',
    'core/api/utils/Exceptions.kt',
    'core/api/StartupListener.kt',
    'core/api/ContentInterface.kt',
    'core/game/system/config/DropTableParser.kt',
    'content/data/tables/RareDropTable.kt',
    'content/data/tables/AllotmentSeedDropTable.java',
    'content/data/tables/CELEMinorTable.java',
    'content/data/tables/GemDropTable.java',
    'content/data/tables/HerbDropTable.java',
    'content/data/tables/RareSeedDropTable.java',
    'content/data/tables/UncommonSeedDropTable.java',
    'core/game/node/entity/npc/drop/NPCDropTables.java',
    'core/game/node/entity/npc/drop/DropFrequency.java',
    'core/game/node/item/ChanceItem.java',
    'core/game/node/item/WeightedChanceItem.java',
    'core/tools/RandomFunction.java',
]
SHARED = ['RDT', 'CELEDT', 'USDT', 'HDT', 'GDT', 'RSDT', 'ASDT']
PINNED = ['drop_tables.json'] + [f'shared_tables/{name}.xml' for name in SHARED]
SCOPE = (
    'Unchanged WeightBasedTable/WeightedItem/NPCDropTable/DropTableParser/RareDropTable, the six shared '
    'weighted-chance tables, NPCDropTables, DropFrequency and RandomFunction on JDK 11. The complete pinned '
    'drop_tables.json and shared_tables/*.xml, designed parser, table, hand-off and boundary-draw cases. '
    'Each case seeds RandomFunction.RANDOM, the generator private to Collections.shuffle(List) and the '
    "thread's ThreadLocalRandom (the last two through reflection) and records how far each advanced per "
    "action. The rare table's guaranteed list is filled by its own add, reached through reflection on the "
    'private table. NPCDefinition lookup, clue registration, ring-of-wealth and glove queries, glove charges, the '
    'NPC behaviour callback, inventory space, item names and logging are recording adapters; Item keeps its '
    'constructor arguments. getClue, exceptionToString and the Log enum are original text inserted at '
    'generation time. Log text after the first line (JVM stack frames) is dropped. createDrop, loot share, '
    'bone crusher, drop messages, ground items and NPC death are not exercised.'
)

MASK64 = (1 << 64) - 1
GAMMA = 0x9E3779B97F4A7C15
INT_MIN, INT_MAX = -(1 << 31), (1 << 31) - 1


def bits(value):
    """IEEE-754 bits of a double as the signed long the harness reads."""
    return struct.unpack('<q', struct.pack('<d', value))[0]


def signed(value):
    value &= MASK64
    return value - (1 << 64) if value >> 63 else value


def unmix64(z):
    """Inverse of ThreadLocalRandom.mix64."""
    z ^= z >> 33
    z = (z * pow(0xC4CEB9FE1A85EC53, -1, 1 << 64)) & MASK64
    z ^= z >> 33
    z = (z * pow(0xFF51AFD7ED558CCD, -1, 1 << 64)) & MASK64
    return z ^ (z >> 33)


def tlr_seed(r53):
    """Seed whose next nextDouble uses the 53-bit value r53 (nextLong() >>> 11)."""
    return signed(unmix64((r53 << 11) | 0x2A5) - GAMMA)


class JavaRandom:
    """java.util.Random, used only to choose inputs; the JVM decides every expectation."""

    def __init__(self, seed):
        self.state = (seed ^ 0x5DEECE66D) & ((1 << 48) - 1)

    def next(self, width):
        self.state = (self.state * 0x5DEECE66D + 0xB) & ((1 << 48) - 1)
        return self.state >> (48 - width)

    def next_int(self, bound):
        r = self.next(31)
        if bound & (bound - 1) == 0:
            return (bound * r) >> 31
        while r - r % bound + bound - 1 >= 1 << 31:
            r = self.next(31)
        return r % bound

    def next_long(self):
        high = self.next(32)
        low = self.next(32)
        high -= (high >> 31) << 32
        low -= (low >> 31) << 32
        return signed((high << 32) + low)


def shuffled_order(weights, seed):
    """Order WeightBasedTable.add leaves the given weights in for RandomFunction.RANDOM seeded with seed."""
    random = JavaRandom(seed)
    table = []
    for weight in weights:
        index = random.next_int(len(table)) if table else 0
        table.append(weight)
        table[index], table[-1] = weight, table[index]
    return table


def residue_case():
    """A table whose summed total exceeds what the in-order subtraction reaches: the largest draw drops nothing."""
    picker = JavaRandom(20091130)
    for _ in range(200000):
        count = 3 + picker.next_int(6)
        weights = [(1 + picker.next_int(999)) / 10.0 for _ in range(count)]
        total = 0.0
        for weight in weights:
            total += weight
        draw = ((1 << 53) - 1) * 2.0 ** -53 * total
        if draw >= total:
            continue
        for seed in range(1, 9):
            left = draw
            for weight in shuffled_order(weights, seed):
                left -= weight
            if left > 0:
                return weights, seed
    raise RuntimeError('no rounding residue found')


def requests():
    cases = []

    def a(op, **kw):
        return dict(op=op, **kw)

    def add(actions, **kw):
        cases.append(dict(actions=actions, **kw))

    def w(id, lo=1, hi=1, weight=1.0, guaranteed=False):
        return [id, lo, hi, bits(weight), guaranteed]

    def main(*args, **kw):
        return ['main'] + w(*args, **kw)

    def charm(*args, **kw):
        return ['charm'] + w(*args, **kw)

    def tertiary(*args, **kw):
        return ['tertiary'] + w(*args, **kw)

    def table(name, adds, kind='weighted'):
        return a('table', name=name, kind=kind, adds=adds)

    def roll(name, times=1, receiver=None):
        return a('rollTable', table=name, times=times, receiver=receiver)

    def seeds(random=None, shuffle=None, tlr=None):
        return {key: value for key, value in dict(random=random, shuffle=shuffle, tlr=tlr).items() if value is not None}

    def shared(name, text):
        return a('shared', table=name, text=text)

    def xml(rows, root='T'):
        return f'<{root}>' + ''.join(f'<item id="{i}" minAmt="{lo}" maxAmt="{hi}" weight="{weight}"/>' for i, lo, hi, weight in rows) + f'</{root}>'

    ENTITY = dict(kind='entity')
    PLAYER = dict(kind='player')
    WEALTH = dict(kind='player', wealth=True)

    def gloves(*ids, **kw):
        return dict(kind='player', equipped=list(ids), **kw)

    RECEIVERS = [None, ENTITY, PLAYER, WEALTH, gloves(12856), gloves(12859, wealth=True)]
    CLUES = dict(EASY=[2677, 2678, 2679, 2680, 2682], MEDIUM=[2801, 2803, 2805], HARD=[2722])
    pinned_shared = [a('shared', table=name, pinned=True) for name in SHARED]
    RDT, EASY, MEDIUM, HARD, CELE, USDT, HDT, GDT, RSDT, ASDT = 31, 1, 5733, 12070, 799, 14422, 14424, 14426, 14428, 14430
    picker = JavaRandom(530)

    # Constants and the two NPCDropTables value helpers.
    add([a('constants')])
    add([a('stabilizer', combat=level) for level in [INT_MIN, -3, -2, -1, 0, 1, 2, 9, 138, INT_MAX - 1, INT_MAX]])
    add([a('modRate', bits=bits(value)) for value in [0.0, 1.5, -0.0, float('inf')]] + [a('modRate', bits=bits(float('nan')))])

    # RandomFunction, the calls the drop path makes.
    pairs = [(0, 0), (5, 5), (0, 1), (1, 0), (1, 2), (3, 10), (10, 3), (-5, 5), (5, -5), (-7, -7), (-10, -3), (0, INT_MAX),
             (INT_MIN, INT_MAX), (INT_MAX, INT_MIN), (0, INT_MIN), (INT_MIN, 0), (1, INT_MIN), (-1, INT_MAX), (0, 1 << 30),
             (0, (1 << 30) + 1), (0, 1610612736), (-1610612736, 0), (7, 1610612743)]
    for seed in [0, 1, 530, -7, 1 << 40]:
        add([a('random', fn='range', a=lo, b=hi) for lo, hi in pairs], seeds=seeds(random=seed))
        add([a('random', fn='max', n=n) for n in [-1, 0, 1, 2, 3, 7, 8, 100, 1 << 30, (1 << 30) + 1, 1610612736, INT_MAX, INT_MIN]], seeds=seeds(random=seed))
        add([a('random', fn='bool') for _ in range(16)], seeds=seeds(random=seed))
    bounds = [1.0, 8.0, 0.1, 100.0, 3.0, 1.0 + 2.0 ** -52, 5e-324, 1e-320, 2.2250738585072014e-308, 1.7976931348623157e308,
              float('inf'), 0.0, -0.0, -1.0, float('-inf'), float('nan'), 171.0, 0.30000000000000004]
    for bound in bounds:
        steps = []
        for r53 in [0, 1, 1 << 52, (1 << 53) - 2, (1 << 53) - 1, (picker.next(27) << 26) | picker.next(26)]:
            steps += [a('seed', tlr=tlr_seed(r53)), a('random', fn='double', bits=bits(bound))]
        add(steps)
    add([a('random', fn='double', bits=bits(8.0)) for _ in range(12)], seeds=seeds(tlr=-1))

    # WeightBasedTable.add: insertion order, the swap and the total.
    for seed in [0, 1, 2, 530, -99]:
        for count in [0, 1, 2, 3, 5, 9, 17]:
            adds = [main(100 + i, 1, 1 + i, 1.0 + i) for i in range(count)]
            add([table('t', adds), a('snapshot', table='t')], seeds=seeds(random=seed))
    add([table('t', [main(1, 1, 1, 0.1), main(2, 1, 1, 0.2, guaranteed=True), main(3, 1, 1, 0.3), main(4, 2, 5, 0.0, guaranteed=True), main(5, 1, 1, 0.4)])])
    add([table('t', [main(i, 1, 1, weight) for i, weight in enumerate([0.1, 0.2, 0.3, 0.7, 1e16, 1.0, -3.5, 5e-324])])])
    add([table('t', [main(1, 1, 1, float('nan')), main(2, 1, 1, 1.0)]), roll('t')])
    add([table('t', [main(1, 1, 1, float('inf')), main(2, 1, 1, 1.0), main(3, 1, 1, 2.0)])] + [roll('t') for _ in range(6)])
    add([table('t', [main(4151, 1, 1, 2.0), main(995, 5, 50, 6.0), main(1, 1, 1, 0.5, guaranteed=True)], kind='created'), a('snapshot', table='t')])
    add([table('t', [])], seeds=seeds(random=7))
    add([table('t', [main(995, 1, 1, 4.0)] + [['insert', name, bits(1.0 + i)] for i, name in enumerate(
        ['EasyClue', 'MediumClue', 'HardClue', 'RDTRoll', 'CELEDTRoll', 'SEEDDTRoll', 'HERBDTRoll', 'GDTRoll', 'RSDTRoll', 'ASDTRoll'])]),
        a('snapshot', table='t')], seeds=seeds(random=11))
    add([table('n', [main(1, 1, 1, 1.0), charm(12158, 1, 1, 3.0), tertiary(2, 1, 1, 5.0), charm(12159, 1, 1, 4.0), main(3, 1, 1, 2.0, guaranteed=True),
                     tertiary(4, 1, 1, 1.0, guaranteed=True), charm(12160, 1, 1, 1.0, guaranteed=True), main(5, 1, 3, 2.5)], kind='npc'), a('snapshot', table='n')])

    # Weighted roll: every cumulative boundary of a dyadic table, one unit either side.
    for seed in [3, 4, 5]:
        steps = [table('t', [main(100, 1, 1, 1.0), main(200, 1, 1, 2.0), main(500, 1, 1, 5.0)]), a('snapshot', table='t')]
        for eighth in range(9):
            for delta in [-1, 0, 1]:
                r53 = eighth * (1 << 50) + delta
                if 0 <= r53 < 1 << 53:
                    steps += [a('seed', tlr=tlr_seed(r53)), roll('t')]
        add(steps, seeds=seeds(random=seed))
    for seed in [1, 2]:
        steps = [table('t', [main(100 + i, 1, 1, 0.5) for i in range(4)] + [main(900, 1, 1, 0.0)])]
        for quarter in range(5):
            for delta in [-1, 0, 1]:
                r53 = quarter * (1 << 51) + delta
                if 0 <= r53 < 1 << 53:
                    steps += [a('seed', tlr=tlr_seed(r53)), roll('t')]
        add(steps, seeds=seeds(random=seed))
    weights, seed = residue_case()
    add([table('t', [main(100 + i, 1, 1, weight) for i, weight in enumerate(weights)]), a('snapshot', table='t'),
         a('seed', tlr=tlr_seed((1 << 53) - 1)), roll('t'), a('seed', tlr=tlr_seed((1 << 53) - 2)), roll('t'),
         a('seed', tlr=tlr_seed(0)), roll('t')], seeds=seeds(random=seed), note='rounding residue')
    for seed in range(6):
        steps = [table('t', [main(100 + i, 1, 4, weight) for i, weight in enumerate([0.1, 0.2, 0.7, 2.5, 13.0, 0.01])])]
        steps += [roll('t', times) for times in [1, 1, 1, 2, 3, 5, 0, -1, 1, 1]] + [a('rollTable', table='t', receiver=None)]
        add(steps, seeds=seeds(random=seed, tlr=1000 + seed))
    # Empty, single-entry, guaranteed-only and unusable totals.
    add([table('t', []), roll('t'), roll('t', 5), a('rollTable', table='t', receiver=PLAYER)])
    for weight in [1.0, 0.25, 1e-300]:
        add([table('t', [main(995, 3, 9, weight)]), roll('t'), roll('t', 3)], seeds=seeds(tlr=signed(picker.next_long())))
    add([table('t', [main(995, 3, 9, 1.0, guaranteed=True), main(526, 1, 1, 1.0, guaranteed=True)]), roll('t'), roll('t', 3), roll('t', 0)])
    for weight in [0.0, -1.0, -0.0]:
        add([table('t', [main(995, 1, 1, weight), main(526, 2, 3, 1.0, guaranteed=True)]), roll('t'), roll('t', 2)])
    add([table('t', [main(995, 1, 1, 2.0), main(526, 1, 1, -2.0)]), roll('t')])
    add([table('t', [main(995, 1, 1, 2.0), main(526, 1, 1, -1.0), main(4151, 1, 1, 0.5)])] + [roll('t') for _ in range(8)])
    # Amount ranges, including ones the parser would refuse.
    for lo, hi in [(1, 1), (0, 0), (1, 2), (5, 50), (50, 5), (-5, 5), (-9, -2), (0, INT_MAX), (INT_MIN, INT_MAX), (INT_MAX, INT_MIN), (0, INT_MIN), (7, 1610612743)]:
        add([table('t', [main(995, lo, hi, 1.0), main(526, lo, hi, 1.0, guaranteed=True)]), roll('t'), roll('t', 3)], seeds=seeds(random=lo ^ hi))

    # convertWeightedItems: every slot, every kind of receiver.
    slots = [EASY, MEDIUM, HARD, RDT, CELE, USDT, HDT, GDT, RSDT, ASDT]
    for receiver in RECEIVERS:
        for index, clues in enumerate([CLUES, {}, dict(EASY=[2677], HARD=[2722, 2723])]):
            steps = [table('t', [])] + (pinned_shared if index != 1 else [])
            steps += [a('convert', table='t', items=[w(slot)], receiver=receiver) for slot in slots]
            steps += [a('convert', table='t', items=[w(slot, 2, 6, 3.0, True) for slot in slots] + [w(0), w(995, 10, 20), w(0, 5, 9), w(4151)], receiver=receiver)]
            add(steps, clues=clues, seeds=seeds(random=40 + index, shuffle=50 + index, tlr=60 + index))
    add([table('t', []), a('convert', table='t', items=[], receiver=None), a('convert', table='t', items=[w(0), w(0, 1, 9)], receiver=PLAYER),
         a('convert', table='t', items=[w(995, 1, 9), w(-1, 1, 9), w(65536, 2, 2), w(INT_MAX, 1, 3), w(INT_MIN, -3, -1)], receiver=ENTITY)])
    add([table('t', []), a('clue', level='EASY'), a('clue', level='MEDIUM'), a('clue', level='HARD'), a('clue', level='UNKNOWN'),
         a('clues', level='EASY', ids=[]), a('clue', level='EASY'), a('clues', level='EASY', ids=None), a('clue', level='EASY'),
         a('clues', level='UNKNOWN', ids=[7, 8, 9]), a('clue', level='UNKNOWN')] + [a('clue', level='MEDIUM') for _ in range(12)], clues=CLUES)

    # Herb slot: the coin flip, the receiver and each glove, in the original's order.
    herb_receivers = [None, ENTITY, PLAYER, WEALTH, gloves(12856), gloves(12857), gloves(12858), gloves(12859), gloves(12859, 12856),
                      gloves(12858, 12857), gloves(12856, 12857, 12858, 12859), gloves(209, 2572)]
    for seed in [0, 1, 2, 3]:
        for empty in [False, True]:
            steps = [table('t', [])] + ([] if empty else [a('shared', table='HDT', pinned=True)])
            for receiver in herb_receivers:
                steps += [a('convert', table='t', items=[w(HDT)], receiver=receiver), a('convert', table='t', items=[w(HDT), w(HDT), w(HDT)], receiver=receiver)]
            add(steps, seeds=seeds(random=seed, shuffle=seed + 9))

    # Special slots inside roll: only the herb and uncommon-seed slots repeat by amount.
    for seed in [0, 1, 2]:
        adds = [main(HDT, 2, 4, 1.0, guaranteed=True), main(USDT, 0, 0, 1.0, guaranteed=True), main(USDT, -3, -1, 1.0, guaranteed=True),
                main(GDT, 2, 4, 1.0, guaranteed=True), main(995, 2, 4, 1.0, guaranteed=True), main(USDT, 4, 10, 1.0), main(HDT, 8, 10, 1.0), main(RSDT, 4, 10, 1.0)]
        add(pinned_shared + [table('t', adds)] + [roll('t', times, receiver) for times in [1, 2] for receiver in [None, PLAYER, gloves(12857)]],
            seeds=seeds(random=seed, shuffle=seed + 100, tlr=seed + 200))
    add(pinned_shared + [table('t', [main(USDT, 4, 10, 1.0)]), roll('t'), roll('t', 3), table('u', [main(HDT, 3, 3, 1.0)]), roll('u'), roll('u', 2, PLAYER)])

    # Rare drop table: empty, single entry, nothing entries, the ring of wealth, nested and self hand-offs.
    rare = [a('convert', table='t', items=[w(RDT)], receiver=receiver) for receiver in [None, ENTITY, PLAYER, WEALTH]]
    add([table('t', [])] + rare + [a('retrieve', table='RDT', receiver=WEALTH)])
    for rows in [[(995, 3, 9, 12)], [(0, 1, 1, 171)], [(0, 1, 1, 5), (0, 2, 2, 7)], [(995, 3, 9, 12), (0, 1, 1, 171)], [(0, 1, 1, 171), (995, 3, 9, 12)],
                 [(995, 1, 1, 0), (526, 1, 1, 0)], [(995, 1, 1, 1), (526, 1, 1, 1), (0, 1, 1, 2)], [(995, 1, 1, -4), (526, 1, 1, 1), (0, 1, 1, 9)]]:
        for seed in [0, 1]:
            add([table('t', []), shared('RDT', xml(rows, 'RDT'))] + rare * 3 + [a('sharedSnapshot', table='RDT')], seeds=seeds(random=seed, shuffle=seed + 5, tlr=seed + 6))
    nested = [(GDT, 1, 1, 3), (HDT, 1, 1, 3), (USDT, 1, 1, 3), (CELE, 1, 1, 3), (RSDT, 1, 1, 3), (ASDT, 1, 1, 3), (EASY, 1, 1, 3), (MEDIUM, 1, 1, 3),
              (HARD, 1, 1, 3), (RDT, 1, 1, 6), (995, 100, 200, 3), (0, 1, 1, 10)]
    for seed in range(4):
        add(pinned_shared + [shared('RDT', xml(nested, 'RDT'))] + [table('t', [])] + [
            a('retrieve', table='RDT', receiver=receiver) for _ in range(12) for receiver in [None, WEALTH, gloves(12858)]],
            clues=CLUES, seeds=seeds(random=seed, shuffle=seed + 20, tlr=seed + 30))
    for seed in range(3):
        add([a('shared', table='RDT', pinned=True), table('t', [])] + [a('retrieve', table='RDT', receiver=receiver) for _ in range(40) for receiver in [None, PLAYER, WEALTH]],
            seeds=seeds(random=seed, shuffle=seed + 1, tlr=seed + 2))
    # Rare table boundaries: the draw is taken before the shuffle, and nothing entries are passed over.
    for seed in [0, 1]:
        steps = [shared('RDT', xml([(100, 1, 1, 1), (200, 1, 1, 2), (0, 1, 1, 4), (500, 1, 1, 1)], 'RDT')), table('t', [])]
        for eighth in range(9):
            for delta in [-1, 0, 1]:
                r53 = eighth * (1 << 50) + delta
                if 0 <= r53 < 1 << 53:
                    steps += [a('seed', tlr=tlr_seed(r53)), a('retrieve', table='RDT', receiver=None),
                              a('seed', tlr=tlr_seed(r53)), a('retrieve', table='RDT', receiver=WEALTH)]
        add(steps, seeds=seeds(random=seed, shuffle=seed + 77))

    # The six weighted-chance tables: their in-place shuffle persists between calls.
    chance = [n for n in SHARED if n != 'RDT']
    for name in chance:
        add([a('retrieve', table=name), a('shared', table=name, pinned=True)] + [a('retrieve', table=name) for _ in range(30)] + [a('sharedSnapshot', table=name)],
            seeds=seeds(random=len(name), shuffle=len(name) + 1))
    for rows in [[(995, 1, 1, 1)], [(995, 3, 9, 1)], [(995, 1, 1, 0)], [(995, 1, 1, 0), (526, 1, 1, 0)], [(995, 1, 1, 5), (526, 2, 4, 1), (4151, 1, 1, 0)],
                 [(995, 1, 1, -5), (526, 2, 4, 2)], [(995, 1, 1, INT_MAX), (526, 2, 4, 2)], [(i, 1, i, i) for i in range(1, 9)],
                 [(995, 5, 2, 3), (526, -4, 4, 3), (0, 1, 1, 3), (RDT, 1, 1, 3)]]:
        for seed in [0, 1]:
            add([shared('GDT', xml(rows))] + [a('retrieve', table='GDT') for _ in range(10)] + [a('sharedSnapshot', table='GDT')], seeds=seeds(random=seed, shuffle=seed + 3))
    add([shared(name, xml([(index + 1, 1, 3, 2), (index + 50, 1, 1, 1)])) for index, name in enumerate(chance)]
        + [a('retrieve', table=name) for _ in range(4) for name in chance] + [a('sharedSnapshot', table=name) for name in chance])

    # Shared-table files: partial loads, the element and attribute rules, and startup.
    good = '<item id="995" minAmt="1" maxAmt="2" weight="3"/>'
    documents = [
        '', 'not xml', '<T/>', '<T></T>', f'<T>{good}', f'<T>{good}</T>', f'<T>{good}{good}</T>', f'<?xml version="1.0" encoding="UTF-8"?>\n<T>\n  {good}\n</T>\n',
        f'<T>{good}<item id="526" minAmt="1" maxAmt="2"/>{good}</T>', f'<T>{good}<item minAmt="1" maxAmt="2" weight="3"/>{good}</T>',
        f'<T>{good}<item id="526" maxAmt="2" weight="3"/></T>', f'<T>{good}<item id="526" minAmt="2" weight="3"/></T>',
        '<T><item id="1.5" minAmt="1" maxAmt="1" weight="1"/></T>', '<T><item id=" 5" minAmt="1" maxAmt="1" weight="1"/></T>',
        '<T><item id="+5" minAmt="-1" maxAmt="+1" weight="-7"/></T>', '<T><item id="" minAmt="1" maxAmt="1" weight="1"/></T>',
        '<T><item id="5" minAmt="1" maxAmt="1" weight="2147483648"/></T>', '<T><item id="5" minAmt="1" maxAmt="1" weight="1.0"/></T>',
        '<T><item id="5" minAmt="9" maxAmt="1" weight="1"/></T>', "<T><item id='5' minAmt='1' maxAmt='1' weight='1'/></T>",
        '<T><item id="&#53;" minAmt="1" maxAmt="1" weight="1"/></T>', '<T><item id="5"\n minAmt="1"\tmaxAmt="1" weight="1"></item></T>',
        f'<T><group>{good}<deeper>{good}</deeper></group><ITEM id="1" minAmt="1" maxAmt="1" weight="1"/><items id="2"/>{good}</T>',
        f'<T><item id="7" minAmt="1" maxAmt="1" weight="1">{good}</item></T>', f'<T><!-- {good} --><![CDATA[{good}]]>{good}</T>',
        '<item id="5" minAmt="1" maxAmt="1" weight="1"/>', f'<T xmlns="urn:x">{good}</T>',
        f'<T xmlns:x="urn:x"><x:item id="1" minAmt="1" maxAmt="1" weight="1"/>{good}<item x:id="2" id="3" minAmt="1" maxAmt="1" weight="1"/></T>',
        f'<T>{good}</T><T/>', f'<T>{good}<broken></T>', f'<T>{good}<item id="1" id="2" minAmt="1" maxAmt="1" weight="1"/></T>',
        '<T><item id="٥" minAmt="1" maxAmt="1" weight="1"/></T>', f'<!DOCTYPE T [<!ENTITY five "5">]><T><item id="&five;" minAmt="1" maxAmt="1" weight="1"/></T>',
    ]
    for document in documents:
        add([shared('HDT', document), shared('RDT', document), shared('HDT', document), a('retrieve', table='HDT'), a('retrieve', table='RDT', receiver=None)])
    for name in SHARED:
        add([a('startup', table=name, path='present', pinned=True), a('startup', table=name, path='missing'), a('startup', table=name, path='null'),
             a('startup', table=name, path='present', text='<T><item id="5"/></T>'), a('sharedSnapshot', table=name)])

    # NPCDropTable: charms, then tertiary, then main, each for every requested time.
    npc_adds = [main(995, 5, 50, 6.0), main(RDT, 1, 1, 1.0), main(HDT, 1, 2, 1.0), main(526, 1, 1, 1.0, guaranteed=True), main(HARD, 1, 1, 0.5),
                charm(12158, 1, 1, 3.0), charm(12159, 1, 2, 2.0), charm(0, 1, 1, 14.0), charm(12163, 1, 1, 0.0),
                tertiary(MEDIUM, 1, 1, 1.0), tertiary(0, 1, 1, 30.0), tertiary(8839, 1, 1, 0.25)]
    for seed in range(5):
        steps = pinned_shared + [table('n', npc_adds, kind='npc'), a('snapshot', table='n')]
        steps += [roll('n', times, receiver) for times in [1, 3, 0] for receiver in RECEIVERS] + [a('rollTable', table='n', receiver=WEALTH)]
        add(steps, clues=CLUES, seeds=seeds(random=seed, shuffle=seed + 300, tlr=seed + 400))
    add([table('n', [charm(1, 1, 1, 0.0), main(995, 1, 1, 1.0)], kind='npc'), roll('n'), table('m', [tertiary(1, 1, 1, -1.0), main(995, 1, 1, 1.0)], kind='npc'), roll('m'),
         table('o', [charm(12158, 2, 3, 1.0), main(995, 1, 1, 0.0)], kind='npc'), roll('o')])

    # NPCDropTables.roll and the behaviour callback that receives the list.
    for behavior in [None, dict(removeFirst=True), dict(add=[4151, 2]), dict(removeFirst=True, add=[6199, 1]), dict(fail=True)]:
        add([table('n', npc_adds, kind='npc'), a('assign', npc=50, table='n')]
            + [a('roll', npc=50, times=times, receiver=receiver, behavior=behavior) for times in [1, 2] for receiver in [PLAYER, ENTITY, None]],
            clues=CLUES, definitions=dict(names={'50': 'King Black Dragon'}))
    add([a('roll', npc=1, times=1, receiver=PLAYER), a('roll', npc=1, times=3, receiver=None), a('tables')])

    # excluding, canRoll and toString.
    base = [main(995, 5, 50, 6.0), main(EASY, 1, 1, 1.0), main(MEDIUM, 1, 1, 1.0), main(HARD, 1, 1, 1.0), main(526, 1, 2, 1.0, guaranteed=True),
            main(HARD, 1, 1, 1.0, guaranteed=True), main(4151, 1, 1, 0.125)]
    for kind in ['weighted', 'npc']:
        extra = [charm(12158, 1, 1, 3.0), tertiary(8839, 1, 1, 0.25)] if kind == 'npc' else []
        add([table('t', base + extra, kind=kind), a('excluding', table='t', ids=[EASY, MEDIUM, HARD], **{'as': 'u'}), a('excluding', table='t', ids=[], **{'as': 'v'}),
             a('excluding', table='u', ids=[995, 526, 4151], **{'as': 'x'}), a('excluding', table='t', ids=[995, 995, -1], **{'as': 'y'}),
             roll('u', 2, PLAYER), roll('x'), a('snapshot', table='t')], clues=CLUES, seeds=seeds(random=9))
    for hasSpace in [False, True]:
        for full in [False, True]:
            receiver = dict(kind='player', hasSpace=hasSpace, full=full)
            add([table('t', base), a('canRoll', table='t', receiver=receiver), table('u', [main(995, 1, 1, 1.0)]), a('canRoll', table='u', receiver=receiver),
                 table('v', []), a('canRoll', table='v', receiver=receiver), table('n', base + [charm(1, 1, 9, 1.0, guaranteed=True)], kind='npc'),
                 a('canRoll', table='n', receiver=receiver)])
    add([table('t', base), a('describe', table='t'), a('describe', table='t', names={'995': 'Coins', '4151': None, '1': ''}), table('u', []), a('describe', table='u'),
         table('v', [main(1, INT_MIN, INT_MAX, value) for value in [0.0, -0.0, 1e-5, 0.001, 1234567.0, 1e7, 1.0 / 3.0, float('nan'), float('inf'), 5e-324]]), a('describe', table='v')])

    # DropTableParser.
    def entry(id='995', lo='1', hi='1', weight='1.0', **kw):
        row = {}
        for key, value in [('id', id), ('minAmount', lo), ('maxAmount', hi), ('weight', weight)]:
            if value is not ...:
                row[key] = value
        row.update(kw)
        return row

    def row(ids='1', main=..., charm=..., default=..., tertiary=..., **kw):
        out = dict(description='')
        for key, value in [('ids', ids), ('main', main), ('charm', charm), ('default', default)]:
            if value is ...:
                value = '1' if key == 'ids' else []
            if value is not None or key in kw.get('nulls', ()):
                out[key] = value
        if tertiary is not ...:
            out['tertiary'] = tertiary
        out.update({k: v for k, v in kw.items() if k != 'nulls'})
        return out

    def load(value, **kw):
        text = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)
        add([a('load', text=text), a('tables')], **kw)

    for text in ['', ' ', 'null', '{}', '[]', '[null]', '[3]', '["1"]', '[[]]', '[{}]', '[', '[{"ids":"1"}', '{"ids":"1"}', '3', '"x"', 'true',
                 '[{"ids":"1","main":[],"charm":[],"default":[]},]', '[,{"ids":"1","main":[],"charm":[],"default":[]}]',
                 '[{"ids":"1","main":[],"charm":[],"default":[]} {"ids":"2","main":[],"charm":[],"default":[]}]',
                 '[{"ids":"1","ids":"2","main":[],"charm":[],"default":[],"main":[{"id":"995","minAmount":"1","maxAmount":"1","weight":"2.5"}]}]']:
        load(text)
    add([a('load', missingFile=True), a('tables')])
    sample = [entry('995', '5', '50', '6.0'), entry('526'), entry('4151', '1', '1', '0.125'), entry('1163', '1', '1', '2.0')]
    for missing in ['ids', 'main', 'charm', 'default', 'tertiary', 'description']:
        full = row('1,2', sample, [entry('12158')], [entry('526')], [entry('8839')])
        del full[missing]
        load([full, row('3', sample)])
    for key in ['main', 'charm', 'default', 'tertiary']:
        for value in [None, {}, 'x', 3, True, [None], [3], [[]], [{}], ['x']]:
            full = row('1,2', sample, [entry('12158')], [entry('526')], [entry('8839')])
            full[key] = value
            load([full, row('3', sample)])
    for ids in ['1', '1,2,3', '1,1', '3,2,1,2', '', ',', '1,', ',1', '1,,2', ' 1', '1 ', '1, 2', '+1,-1,007', '2147483647', '2147483648', '1.0', 'x', 'null',
                '١', None, 5, 5.0, True, ['1', '2'], {}, {'a': 1}, [], [7]]:
        load([row(ids, sample, [entry('12158')], [entry('526')], nulls=('ids',)), row('9', sample)])
    for field in ['id', 'minAmount', 'maxAmount']:
        for value in ['7', 7, 7.0, '7.0', True, None, ..., '+7', '-7', ' 7', '7 ', '٧', '2147483647', '2147483648', '-2147483648', '0x7', '', 'x', {}, [], [7]]:
            item = entry(hi='9')
            if value is ...:
                del item[field]
            else:
                item[field] = value
            load([row('1', sample + [item]), row('2', sample)])
    for value in ['1', 1, 1.5, '1.5', '1e2', '1E-2', ' 2.5 ', '\t2.5\n', '0x1p3', '0x1.8p1', 'NaN', 'Infinity', '-Infinity', '+3', '-1', '-0.0', '0', '1d', '1f', '1D',
                  '1F', '1L', '.5', '5.', '1e', 'e1', '', ' ', 'x', 'nan', 'infinity', True, None, ..., {}, [], [1.5], '4.9e-324', '1e400', '1,5', '1_0', '١',
                  9007199254740993, 1e22, '0.1', '0.30000000000000004']:
        item = entry()
        if value is ...:
            del item['weight']
        else:
            item['weight'] = value
        load([row('1', sample + [item] + sample, [item, entry('12158')], [item]), row('2', sample)])
    # A minimum above the maximum is the one caught failure: the row is dropped and loading continues.
    bad = entry('526', '9', '2')
    for section in ['main', 'charm', 'default', 'tertiary']:
        for position in [0, 2]:
            parts = dict(main=list(sample), charm=[entry('12158'), entry('12159', weight='2.0'), entry('12160')], default=[entry('526'), entry('532'), entry('536')],
                         tertiary=[entry('8839'), entry('8840'), entry('8841')])
            parts[section].insert(position, bad)
            load([row('1', sample), row('2,3', **parts), row('4', sample), row('1', sample[:2])], definitions=dict(names={'2': 'Goblin'}))
    for ids, names in [('2', {'2': None}), ('x,2', {}), ('', {}), (None, {}), ('2147483648', {}), (' 2', {}), ('-5,2', {'-5': 'Negative'})]:
        load([row('1', sample), row(ids, [bad]), row('4', sample)], definitions=dict(names=names))
    load([row('1', [entry('526', '9', '2', 'x')]), row('2', [entry('526', '9', 'x', '1.0')]), row('3', sample)])
    load([row('1', [entry('526', '2', '2'), entry('526', '-3', '-3'), entry('526', '-3', '-2'), entry('526', '0', '0')]), row('2', [entry('526', '-2', '-3')])])
    load([row('1', sample, [entry('12158')], [entry('526')], [entry('8839')], extra=1, description=None), row('2', sample, description=7)])
    # Lookup results the parser reads: null drop tables, a failing lookup, a null definition.
    for options in [dict(nullTables=[2]), dict(nullTables=[1, 2, 3]), dict(throwing=[2]), dict(throwing=[1]), dict(missing=[2]), dict(missing=[3], nullTables=[1])]:
        load([row('1,2,3', sample), row('4', sample)], definitions=options)
    load([row('1', [bad]), row('4', sample)], definitions=dict(throwing=[1]))
    load([row('1', [bad]), row('4', sample)], definitions=dict(missing=[1]))
    # A later row replaces an earlier table; ids of one row share one table.
    load([row('1,2,3', sample), row('2', sample[:1]), row('3,4', []), row('2', sample[2:], [entry('12158')])])
    for seed in [0, 1, 2, 3, 530, -1]:
        many = [entry(str(100 + i), '1', str(1 + i % 4), str(0.5 + i * 0.25)) for i in range(24)]
        add([a('load', text=json.dumps([row('1,2', many, many[:9], many[:3], many[:5]), row('3', many[5:])])), a('tables'),
             a('roll', npc=1, times=3, receiver=PLAYER), a('roll', npc=3, times=2, receiver=ENTITY)], seeds=seeds(random=seed, tlr=seed))
    add([a('load', text=json.dumps([row('1', sample)])), a('load', text=json.dumps([row('1', sample[:1]), row('2', sample)])), a('tables'), a('load', text='[3]'), a('tables')])

    # The complete pinned files.
    for seed in [530, 2009]:
        add([a('load', pinned=True), a('tables')], seeds=seeds(random=seed))
    pinned = json.loads((FIXTURES / 'drop_tables.json').read_text(encoding='utf-8'))
    ids = [int(text) for entry_ in pinned for text in entry_['ids'].split(',')]
    receivers = [PLAYER, WEALTH, ENTITY, gloves(12856), None, gloves(12859, wealth=True), gloves(12857), gloves(12858)]
    for chunk in range(0, len(ids), 269):
        steps = pinned_shared + [a('load', pinned=True)]
        for offset, id in enumerate(ids[chunk:chunk + 269]):
            # NPCDropTables.roll hands a missing looter to a Kotlin non-null parameter, so every pinned roll names one.
            steps.append(a('roll', npc=id, times=3, receiver=receivers[(chunk + offset) % len(receivers)] or WEALTH))
            steps.append(a('roll', npc=id, times=1, receiver=receivers[(chunk + offset + 3) % len(receivers)] or ENTITY))
        add(steps, clues=CLUES, seeds=seeds(random=chunk + 1, shuffle=chunk + 2, tlr=chunk + 3), pinnedRolls=True)

    # Rare table with guaranteed entries: every selected entry is converted, and draws for it, but only the first item is returned.
    for seed in range(4):
        steps = pinned_shared + [table('t', [])]
        for item in [w(995, 5, 50, 1.0, True), w(HDT, 1, 1, 1.0, True), w(0, 1, 1, 1.0, True), w(MEDIUM, 1, 1, 1.0, True)]:
            steps += [a('rareAdd', item=item)] + [a('retrieve', table='RDT', receiver=receiver) for receiver in [None, WEALTH, gloves(12856)]]
        steps += [a('convert', table='t', items=[w(RDT), w(RDT)], receiver=PLAYER)]
        add(steps, clues=CLUES, seeds=seeds(random=seed + 70, shuffle=seed + 80, tlr=seed + 90))
    add([a('rareAdd', item=w(0, 1, 1, 1.0, True)), a('rareAdd', item=w(4151, 1, 1, 1.0, True)), a('rareAdd', item=w(995, 2, 9, 1.0, True)),
         a('retrieve', table='RDT', receiver=None), a('rareAdd', item=w(526, 1, 1, 3.0)), a('retrieve', table='RDT', receiver=None),
         a('retrieve', table='RDT', receiver=WEALTH)])
    # Equal amounts at the ends of the int range are accepted.
    load([row('1', [entry('526', '2147483647', '2147483647'), entry('527', '-2147483648', '-2147483648'), entry('528', '-2147483648', '2147483647')]),
          row('2', [entry('526', '2147483647', '2147483646')]), row('3', sample)])
    return cases


def artifact(target, lock, caches=()):
    def valid(path):
        return path.is_file() and path.stat().st_size == lock['size'] and hashlib.sha256(path.read_bytes()).hexdigest() == lock['sha256']
    if valid(target):
        return target
    for cache in caches:
        if valid(cache):
            shutil.copyfile(cache, target)
            return target
    with urllib.request.urlopen(lock['url'], timeout=60) as response:
        data = response.read()
    if len(data) != lock['size'] or hashlib.sha256(data).hexdigest() != lock['sha256']:
        raise RuntimeError(f'{target.name} differs from its pin')
    target.write_bytes(data)
    return target


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    reference = ROOT / 'reference/server'
    if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock['server']['commit']:
        raise RuntimeError('Reference pin differs')
    c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    data_hashes = {}
    for name in PINNED:
        raw = c.run(['git', '-C', reference, 'show', f"{lock['server']['commit']}:Server/data/configs/{name}"]).stdout
        target = FIXTURES / name
        if check:
            if target.read_bytes() != raw:
                raise RuntimeError(f'Pinned {name} differs')
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(raw)
        data_hashes[name] = hashlib.sha256(raw).hexdigest()

    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    json_jar = artifact(RUN / 'json-simple-1.1.1.jar', json.loads(c.read(ROOT / 'oracle/auth/json.lock.json')),
                        [ROOT / 'runs/npc-spawns-oracle/json-simple-1.1.1.jar', ROOT / 'runs/npc-definition-oracle/json-simple-1.1.1.jar'])
    consts = artifact(RUN / 'ConstLib-1.4.28.jar', json.loads(c.read(ROOT / 'oracle/bank/constants.lock.json')), [ROOT / 'runs/bank-oracle/ConstLib-1.4.28.jar'])

    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    api = c.read(SERVER / 'core/api/ContentAPI.kt')
    inserted = {
        '/* ORIGINAL_EXCEPTION_TO_STRING */': c.method(api, 'fun exceptionToString ('),
        '/* ORIGINAL_LOG_ENUM */': c.method(c.read(SERVER / 'core/tools/SystemLogger.kt'), 'enum class Log {'),
        '/* ORIGINAL_GET_CLUE */': c.method(c.read(SERVER / 'content/global/activity/ttrail/ClueScrollPlugin.java'), 'public static Item getClue('),
    }
    adapters = RUN / 'adapters'
    if adapters.exists():
        shutil.rmtree(adapters)
    for source in sorted((HERE / 'adapters').rglob('*')):
        if source.is_file():
            text = c.read(source)
            for marker, original in inserted.items():
                text = text.replace(marker, original)
            if 'ORIGINAL_' in text:
                raise RuntimeError(f'Unfilled marker in {source}')
            target = adapters / source.relative_to(HERE / 'adapters')
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding='utf-8', newline='\n')
            (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')

    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    libraries = [stdlib, json_jar, consts]
    c.run(['java', '-Xmx768m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, libraries)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-cp', classpath, '-d', classes, *java])

    cases = requests()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '--add-opens', 'java.base/java.lang=ALL-UNNAMED', '--add-opens',
               'java.base/java.util=ALL-UNNAMED', '-cp', classpath, 'oracle.Support', 'runs/drop-tables-oracle/config',
               'fixtures/drop-tables/drop_tables.json', 'fixtures/drop-tables/shared_tables']
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=600)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace'))
    rows = [json.loads(line) for line in result.stdout.splitlines() if line]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')

    steps = [(step_input, step) for row in rows for step_input, step in zip(row['actions'], row['expected']['steps'])]
    errors = {}
    for _, step in steps:
        if step['error'] is not None:
            errors[step['error']] = errors.get(step['error'], 0) + 1
    events = {}
    for _, step in steps:
        for event in step['events']:
            events[event[0]] = events.get(event[0], 0) + 1
    pinned_rows = json.loads((FIXTURES / 'drop_tables.json').read_text(encoding='utf-8'))
    first = next(row for row in rows if row['actions'][0].get('pinned') and row['actions'][0]['op'] == 'load')
    snapshot = first['expected']['steps'][1]['value']
    rolls = [(i, s) for i, s in steps if i['op'] in ('roll', 'rollTable')]
    residue = next(row for row in rows if row.get('note') == 'rounding residue')
    coverage = dict(
        actions=len(steps),
        rolls=len(rolls),
        rolledItems=sum(len(s['value']) for _, s in rolls if s['value'] is not None),
        pinnedNpcRolls=sum(1 for row in rows if row.get('pinnedRolls') for x in row['actions'] if x['op'] == 'roll'),
        pinnedNpcIdsRolled=len({x['npc'] for row in rows if row.get('pinnedRolls') for x in row['actions'] if x['op'] == 'roll'}),
        randomDraws=sum(s['draws'][0] for _, s in steps if s['draws']),
        shuffleDraws=sum(s['draws'][1] for _, s in steps if s['draws']),
        threadLocalDraws=sum(s['draws'][2] for _, s in steps if s['draws']),
        errors=dict(sorted(errors.items())),
        events=dict(sorted(events.items())),
        residueRollEmpty=residue['expected']['steps'][3]['value'] == [],
    )
    if not coverage['residueRollEmpty']:
        raise RuntimeError('the designed rounding-residue roll dropped an item')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        pinnedDataSha256=data_hashes, pinnedConfigRows=len(pinned_rows), pinnedNpcIds=len(snapshot['npcs']), pinnedTables=len(snapshot['tables']),
        pinnedEntries=sum(len(r[k]) for r in pinned_rows for k in ('main', 'charm', 'default', 'tertiary') if k in r),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', scope=SCOPE)
    summary = (f'{len(rows)} original drop-table cases ({coverage["actions"]} actions, {coverage["rolls"]} rolls), '
               f'{provenance["pinnedConfigRows"]} pinned rows / {provenance["pinnedNpcIds"]} NPC ids / {provenance["pinnedEntries"]} entries')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Drop-table fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
