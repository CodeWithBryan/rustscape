"""Run the unchanged drop and death classes on JDK 11 and record what they do.

Compiled unchanged: NPCDropTables.java, GroundItemManager.java, GroundItem.java,
DeathTask.java, NodeTask.java, Pulse.java, PulseManager.java, PulseType.java,
Bones.kt, StatAttributeKeys.kt and the table classes the roll needs
(WeightBasedTable.kt, WeightedItem.kt, NPCDropTable.kt, RareDropTable.kt, the
six shared tables, DropFrequency.java, RandomFunction.java and the chance
items). PulseRunner.kt is compiled with one added import alias for its clock.
Everything else is an adapter from adapters.py, with the original text of the
methods named there inserted at generation time. --check never updates a
fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import shutil
import struct
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/npc-death-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/npc-death'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('npc_death_adapters', HERE / 'adapters.py')

ORIGINAL = [
    'core/api/utils/WeightBasedTable.kt',
    'core/api/utils/WeightedItem.kt',
    'core/api/utils/NPCDropTable.kt',
    'core/api/utils/Exceptions.kt',
    'core/api/StartupListener.kt',
    'core/api/ContentInterface.kt',
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
    'core/game/node/item/GroundItemManager.java',
    'core/game/node/item/GroundItem.java',
    'core/game/node/entity/combat/DeathTask.java',
    'core/game/system/task/NodeTask.java',
    'core/game/system/task/Pulse.java',
    'core/game/node/entity/impl/PulseManager.java',
    'core/game/node/entity/impl/PulseType.java',
    'content/global/skill/prayer/Bones.kt',
    'core/game/system/command/sets/StatAttributeKeys.kt',
]
SCOPE = (
    'Unchanged NPCDropTables (drop, createDrop, getLooter, sendDropMessage, handleBoneCrusher), GroundItemManager, '
    'GroundItem, DeathTask, NodeTask, Pulse, PulseManager, PulseType, Bones and the table classes of the roll on JDK 11; '
    'PulseRunner with one added import alias for its clock. Original text inserted at generation time: Entity '
    'commenceDeath/finalizeDeath/startDeath; NPC tick, onRespawn, finalizeDeath, commenceDeath, handleDrops, getAudio, '
    'getDropLocation and the respawn accessors; ImpactHandler getMostDamageEntity, its log and disabled-tick accessors, '
    'and the two parts of handleImpact that write the logs and start a death; RegionChunk.add(GroundItem); Item and '
    'ItemDefinition accessors; Definition.getConfiguration; announceIfRare; Location.getDistance; '
    'StringUtils.getFormattedNumber. Recording adapters: every other Entity, NPC and Player member, the behaviour, zone '
    'monitor, clan and loot-share state, Grand Exchange price, item plugins, skills, messages, news, statistics, bots, '
    'packets, the chunk lookup and the list of local and online players. Item definitions, NPC configuration values, '
    'entity hash codes and the tick driver (pulses, then NPC ticks) are case inputs. The roll is seeded as in the '
    'drop-table oracle; no shared table or clue is loaded. Not exercised: DeathTask.getContainers, a dying player\'s own '
    'commenceDeath/finalizeDeath, the rest of handleImpact, ground-item removal and expiry, packet bytes, NPC.clear.'
)

INT_MIN, INT_MAX = -(1 << 31), (1 << 31) - 1
MESSAGE_NPC = 50


def bits(value):
    return struct.unpack('<q', struct.pack('<d', value))[0]


ITEMS = {
    '995': dict(name='Coins', stackable=True, tradeable=True),
    '526': dict(name='Bones', tradeable=True),
    '536': dict(name='Dragon bones', tradeable=True),
    '592': dict(name='Ashes', tradeable=True),
    '1163': dict(name='Rune full helm', tradeable=True),
    '1164': dict(name='Rune full helm', unnoted=False, noteId=1163, tradeable=True),
    '4151': dict(name='Abyssal whip', tradeable=True, rare_item=True),
    '4152': dict(name='Abyssal whip', unnoted=False, noteId=4151, tradeable=True, rare_item=True),
    '561': dict(name='Nature rune', stackable=True, tradeable=True, rare_item=True),
    '562': dict(name='Chaos rune', stackable=True, tradeable=True, rare_item=False),
    '563': dict(name='Law rune', stackable=True, tradeable=True, rare_item=None),
    '564': dict(name='Cosmic rune', stackable=True, tradeable=True, rare_item=7),
    '12158': dict(name='Gold charm', stackable=True, tradeable=False),
    '12163': dict(name='Blue charm', stackable=True),
    '6199': dict(name='Mystery box', tradeable=True),
    '2677': dict(name='Clue scroll', options=[None, None, None, None, 'Destroy'], tradeable=True),
    '4278': dict(name='Ecto-token', stackable=True, tradeable='yes'),
    '11238': dict(name='Baby impling jar', options=[None, None, None, None, 'Destroy'], tradeable=True),
    '7462': dict(name='Barrows gloves'),
    '4675': dict(name='Ancient staff', tradeable=True, plugin=dict(id=7, create=True, item='same')),
    '4676': dict(name='Refused staff', tradeable=True, plugin=dict(id=8, create=False, item='same')),
    '4677': dict(name='Changing staff', tradeable=True, plugin=dict(id=9, create=True, item=[995, 77])),
    '4678': dict(name='Vanishing staff', tradeable=True, plugin=dict(id=10, create=True, item=None)),
    '4679': dict(name='Splitting staff', tradeable=True, plugin=dict(id=11, create=True, item=[1163, 3])),
    '4680': dict(name='Dropped staff', stackable=True, plugin=dict(id=12, create=True, item='same')),
    '14484': dict(name=None, tradeable=True),
    '2996': dict(name='BONE shard', tradeable=True),
    '2997': dict(name='Wishbone', stackable=True, tradeable=True),
    '2998': dict(name='Sacred ASHES', tradeable=True),
    '2999': dict(name='Agility ticket', stackable=True, tradeable=True),
}


def requests():
    cases = []

    def a(op, **kw):
        return dict(op=op, **kw)

    def add(actions, **kw):
        kw.setdefault('items', ITEMS)
        cases.append(dict(actions=actions, **kw))

    def player(**kw):
        return kw

    def npc(id=1, **kw):
        return dict(id=id, name='Man', **kw)

    def main(id, lo=1, hi=1, weight=1.0, guaranteed=False):
        return ['main', id, lo, hi, bits(weight), guaranteed]

    def charm(id, lo=1, hi=1, weight=1.0, guaranteed=False):
        return ['charm', id, lo, hi, bits(weight), guaranteed]

    def tertiary(id, lo=1, hi=1, weight=1.0, guaranteed=False):
        return ['tertiary', id, lo, hi, bits(weight), guaranteed]

    def create(item, player=0, location=(3210, 3210, 0), npc=0):
        return a('createDrop', npc=npc, item=item, player=player, location=None if location is None else list(location))

    HERE_ = [3210, 3210, 0]

    # Constants the native side authors: the bones table and the number format.
    add([a('bones')] + [a('boneFor', id=id) for id in [526, 536, 4834, 3133, 527, 0, -1, 65536 + 526]])
    add([a('format', values=[0, 1, 9, 10, 99, 100, 999, 1000, 1001, 9999, 10000, 123456, 999999, 1000000, 1234567, 12345678, 123456789,
                             1000000000, INT_MAX, -1, -999, -1000, -1234567, INT_MIN])])

    # createDrop: the conditions that end it before anything happens.
    one = [player()]
    for item, who, where in [(None, 0, HERE_), ([0, 5], 0, HERE_), ([65536, 5], 0, HERE_), ([995, 5], 0, None), ([999, 5], 0, HERE_),
                             ([14484, 5], 0, HERE_), ([995, 5], None, HERE_), ([0, 5], None, None), ([14484, 1], None, HERE_),
                             ([14484, 1], 0, None), ([995, 5], 0, HERE_), ([65536 + 995, 5], 0, HERE_), ([526, 1], None, HERE_)]:
        add([create(item, who, where)], players=[player(boneCrusher=True)], npcs=[npc()])

    # Stackable, unstackable and noted items at the amounts that change the path.
    for id in [995, 1163, 1164, 12158]:
        for amount in [1, 2, 3, 28, 0, -1, INT_MIN]:
            add([create([id, amount])], players=one, npcs=[npc()])
    add([create([995, INT_MAX]), create([1164, INT_MAX]), create([995, 65535]), create([995, 65536])], players=one, npcs=[npc()])
    add([create([1163, 300])], players=one, npcs=[npc()])
    for charge_item in [[4151, 1], [4151, 2], [4152, 1], [4152, 2]]:
        add([create(charge_item)], players=[player(name='Zezima')], npcs=[npc()])
    # Several drops on one tile, and a later tick: each ground item keeps its own times.
    add([create([995, 10]), create([995, 20]), a('set', tick=1050), create([1163, 2]), create([995, 30], location=(3211, 3210, 1)),
         a('set', tick=1100), a('groundPulse'), a('set', tick=1150), a('groundPulse'), a('set', tick=1199), a('groundPulse')],
        players=one, npcs=[npc()])
    # An untradeable drop stays private: no public update when the others get theirs.
    add([create([995, 10]), create([7462, 1]), create([2677, 1]), create([11238, 1]), create([12163, 1]), a('set', tick=1100), a('groundPulse')],
        players=one, npcs=[npc()])
    add([create([4278, 1])], players=one, npcs=[npc()])

    # Bone crusher.
    for crusher in [False, True]:
        for item in [[526, 1], [526, 5], [536, 1], [536, 3], [536, 0], [536, -2], [536, INT_MAX], [4834, 28], [592, 1], [995, 5], [65536 + 526, 2]]:
            # Without the perk the original would make two billion single ground items.
            if crusher or item[1] != INT_MAX:
                add([create(item)], players=[player(boneCrusher=crusher)], npcs=[npc()])

    # Item plugins: refused, kept, replaced, replaced by nothing, replaced by an unstackable stack.
    for id in [4675, 4676, 4677, 4678, 4679, 4680]:
        for amount in [1, 4]:
            add([create([id, amount])], players=one, npcs=[npc()])

    # Rare announcements.
    for id in [561, 562, 563, 564, 4151, 4152]:
        for amount in [1, 2, 1000]:
            add([create([id, amount])], players=[player(name='Zezima')], npcs=[npc()])
    add([create([561, 3])], players=[player(name=None)], npcs=[npc()])
    add([create([6199, 1]), create([6199, 3])], players=one, npcs=[npc()])

    # Drop messages: the listed NPCs, the excluded names, the ten-tile circle around the killer.
    for id in [50, 7133, 6227, 2745, 1160, 49, 51, 1, 0, -1, 65536 + 50]:
        add([create([1163, 1]), create([995, 1234567])], players=[player(), player(location=[3203, 3204, 0])], npcs=[npc(id)])
    for id in [526, 536, 592, 2996, 2997, 2998, 2999, 1163]:
        add([create([id, 1])], players=[player(), player()], npcs=[npc(MESSAGE_NPC)])
    spots = [[3200, 3200, 0], [3210, 3200, 0], [3211, 3200, 0], [3206, 3208, 0], [3207, 3208, 0], [3206, 3209, 0], [3193, 3193, 0],
             [3192, 3193, 0], [3200, 3190, 3], [3200, 3189, 3], [3200, 3200, 2], [3190, 3200, 1]]
    for resizable in [False, True]:
        add([create([1163, 1]), create([995, 2]), create([995, 1000])],
            players=[player(resizable=resizable, name='Killer')] + [player(location=spot, resizable=index % 2 == 0) for index, spot in enumerate(spots)],
            npcs=[npc(MESSAGE_NPC)])
    add([create([1163, 1])], players=[player(name=None), player()], npcs=[npc(MESSAGE_NPC)])
    add([create([1163, 1])], players=[player(), player(), player()], repository=[], npcs=[npc(MESSAGE_NPC)])
    add([create([1163, 1])], players=[player(), player(), player()], repository=[2, 1], npcs=[npc(MESSAGE_NPC)])
    add([create([1163, 1])], players=[player(), player(), player()], repository=[1, 0, 1, 0], npcs=[npc(MESSAGE_NPC)])

    # getLooter: every condition both ways.
    def share(**kw):
        base = dict(clan=0, lootShare=True, lootReq=3)
        base.update(kw)
        return base

    members = [share(lsp=10), share(lsp=30), share(lsp=20)]
    variants = [
        ({}, {}, {}),
        (dict(multi=False), {}, {}),
        ({}, dict(clan=None), {}),
        ({}, dict(lootShare=False), {}),
        ({}, dict(lootReq=2), {}),
        ({}, dict(lootReq=1), {}),
        ({}, dict(lootReq=9), {}),
        ({}, dict(ironman=True), {}),
        ({}, dict(resizable=True), {}),
        ({}, {}, dict(local=[0])),
        ({}, {}, dict(local=[])),
        ({}, {}, dict(local=[1])),
        ({}, {}, dict(local=[1, 2])),
        ({}, {}, dict(local=[2, 1, 0])),
        ({}, {}, dict(local=[None, 0, None, 1])),
        ({}, {}, dict(local=[0, 0, 1, 1])),
    ]
    for npc_change, killer_change, extra in variants:
        for item in [[1163, 1], [1164, 5], [12158, 2], [7462, 1], [12163, 1]]:
            players = json.loads(json.dumps(members))
            players[0].update(killer_change)
            case = dict(players=players, clans=[2, 5], local=[0, 1, 2], npcs=[npc(**{**dict(multi=True), **npc_change})], prices={'1163': 20000, '1164': 7},
                        seeds=dict(random=len(cases)))
            case.update(extra)
            add([a('getLooter', npc=0, player=0, item=item), create(item)], **case)
    # The other members' own conditions.
    for change in [dict(clan=None), dict(clan=1), dict(lootShare=False), dict(lootReq=1), dict(lootReq=2), dict(ironman=True), dict(lootReq=4, clan=1),
                   dict(lootReq=5, clan=1)]:
        players = json.loads(json.dumps(members))
        players[1].update(change)
        add([a('getLooter', npc=0, player=0, item=[1163, 1])], players=players, clans=[2, 5], local=[0, 1, 2], npcs=[npc(multi=True)],
            prices={'1163': 20000})
    # The draw: only a member with more points than the highest so far draws, and 0 or 1 keeps the earlier looter.
    for points in [[0, 0, 0], [1, 2, 3], [3, 2, 1], [5, 5, 5], [-1, -2, 7], [1, 1, 2], [INT_MAX, INT_MIN, 0], [0, 1, 0]]:
        for seed in range(6):
            players = [share(lsp=value, resizable=seed % 2 == 0, name=f'Member{index}') for index, value in enumerate(points)]
            add([a('getLooter', npc=0, player=0, item=[1163, 1]), a('getLooter', npc=0, player=1, item=[995, 500]),
                 a('getLooter', npc=0, player=2, item=[12158, 3])],
                players=players, clans=[2], local=[0, 1, 2], npcs=[npc(multi=True)], prices={'1163': 20000, '995': 1}, seeds=dict(random=seed))
    # Prices and the shared points arithmetic.
    for price in [0, 1, 2, 3, 7, 100, 20001, -7, INT_MAX, INT_MIN]:
        for count in [2, 3, 5]:
            players = [share(lsp=1000 * (index + 1), name=f'Member{index}') for index in range(count)]
            add([a('getLooter', npc=0, player=0, item=[1163, 2])], players=players, clans=[2], local=list(range(count)), npcs=[npc(multi=True)],
                prices={'1163': price}, seeds=dict(random=price & 7))
    add([a('getLooter', npc=0, player=0, item=[14484, 1])], players=json.loads(json.dumps(members)), clans=[2], local=[0, 1, 2], npcs=[npc(multi=True)])
    add([a('getLooter', npc=0, player=0, item=[4278, 1])], players=json.loads(json.dumps(members)), clans=[2], local=[0, 1, 2], npcs=[npc(multi=True)])
    add([create([1163, 4]), create([995, 9])], players=json.loads(json.dumps(members)), clans=[2], local=[0, 1, 2], npcs=[npc(multi=True)],
        prices={'1163': 20000, '995': 1})
    # Loot share inside a full drop for a listed NPC.
    for seed in range(4):
        add([create([1163, 1]), create([536, 1]), create([561, 40]), create([6199, 1])],
            players=[share(lsp=5, boneCrusher=True, name='Killer'), share(lsp=50, name='Mate', resizable=True), share(lsp=20, clan=1)],
            clans=[2, 2], local=[1, 0, 2], npcs=[npc(MESSAGE_NPC, multi=True)], prices={'1163': 20000, '561': 300}, seeds=dict(random=seed))

    # Bots: an artificial killer's drop is registered, and a running bot script keeps a list.
    for ai in [False, True]:
        for bot in [None, False, True]:
            add([create([995, 5]), create([1163, 2]), create([1163, 1])], players=[player(ai=ai, bot=bot)], npcs=[npc()])

    # NPCDropTables.drop: roll, behaviour callback, one createDrop per item.
    table = [main(995, 5, 50, 6.0), main(1163, 1, 1, 2.0), main(0, 1, 1, 3.0), main(536, 1, 1, 1.0, True), main(1163, 2, 3, 1.0, True),
             charm(12158, 1, 3, 2.0), charm(0, 1, 1, 5.0), tertiary(561, 10, 20, 1.0), tertiary(0, 1, 1, 9.0)]
    for seed in range(8):
        for looter in ['p0', 'n0', 'n1', None]:
            add([a('drop', npc=0, looter=looter), a('drop', npc=0, looter=looter)], players=[player(boneCrusher=seed % 2 == 1, name='Zezima')],
                npcs=[npc(MESSAGE_NPC, tables=table), npc(2)], seeds=dict(random=seed, tlr=seed + 100))
    for behavior in [dict(removeFirst=True), dict(add=[4151, 1]), dict(removeFirst=True, add=[995, 3]), dict(failAt=1), dict(failAt=2)]:
        add([a('drop', npc=0, looter='p0'), a('drop', npc=0, looter='p0')], players=[player(name='Zezima')], npcs=[npc(tables=table)], behavior=behavior)
    add([a('drop', npc=0, looter='p0')], players=one, npcs=[npc(tables=[])])
    add([a('drop', npc=0, looter='p0')], players=one, npcs=[npc()])
    add([a('drop', npc=0, looter='p0')], players=one, npcs=[npc(tables=[main(995, 1, 1, 0.0)])])
    for place in [None, [3300, 3300, 2]]:
        add([a('drop', npc=0, looter='p0')], players=[player(boneCrusher=True)], npcs=[npc(tables=table, dropLocation=place)], seeds=dict(random=3))
    add([a('drop', npc=0, looter='p0')], players=[player(ai=True, bot=True)], npcs=[npc(tables=table)])

    # Kill credit.
    def hit(source, amount, lethal=False, target='n0'):
        return a('hit', target=target, source=source, amount=amount, lethal=lethal)

    def credit(killer, target='n0'):
        return a('credit', target=target, killer=killer)

    three = [player(uid=7), player(uid=3), player(uid=21)]
    two_npcs = [npc(), npc(2), npc(3)]
    for hits in [[('p0', 5), ('p1', 9), ('p2', 7)], [('p0', 5), ('p1', 5), ('p2', 5)], [('p2', 5), ('p1', 5), ('p0', 5)], [('p0', 4), ('p0', 4), ('p1', 7)],
                 [('p0', 0), ('p1', -3)], [('p0', INT_MAX), ('p0', 1), ('p1', 5)], [('n1', 5), ('n2', 9)], [('n1', 9), ('n2', 9)], [('n2', 9), ('n1', 9)],
                 [('n0', 50), ('n1', 2)], [('n0', 50)], [(None, 4), ('n1', 3)], [(None, 4)], [('n1', 9), ('p0', 1)], []]:
        steps = [hit(source, amount) for source, amount in hits]
        steps += [credit('p0'), credit('n1'), credit(None), credit('p0', target='p1')]
        add(steps, players=three, npcs=two_npcs)
    for hashes in [[1, 17, 33], [33, 17, 1], [16, 32, 48], [5, 4, 3], [65536, 1, 65537], [-1, INT_MIN, INT_MAX]]:
        npcs = [npc(hash=1000), npc(2, hash=hashes[0]), npc(3, hash=hashes[1]), npc(4, hash=hashes[2])]
        add([hit('n1', 5), hit('n2', 5), hit('n3', 5), credit(None), hit('n3', 1), credit(None)], players=three, npcs=npcs)
    for uids in [[1, 17, 33], [33, 17, 1], [16, 0, 32], [-5, 5, 0], [65536, 1, 65537], [INT_MIN, INT_MAX, -1]]:
        add([hit('p0', 5), hit('p1', 5), hit('p2', 5), credit(None), hit('p2', 1), credit(None), a('set', player=2, online=False), credit(None)],
            players=[player(uid=uid) for uid in uids], npcs=[npc()])
    add([hit(f'p{index}', 1 + index % 3) for index in range(20)] + [credit(None)], players=[player(uid=index * 7 % 41) for index in range(20)], npcs=[npc()])
    add([hit('p0', 5), credit('p1'), credit('p0')], players=[player(), player(ai=True)], npcs=[npc()])
    add([hit('p0', 5), a('set', player=0, online=False), credit('p0'), hit('p0', 1, lethal=True), a('tick', count=6), a('tick', count=20, npcs=True)],
        players=one, npcs=[npc(tables=table)])

    # A death from the lethal hit to the respawn.
    def death(ticks=8, after=22):
        return [hit('p0', 5), hit('p0', 3, lethal=True), a('isDead', target='n0'), a('tick', count=ticks), a('isDead', target='n0'),
                a('tick', count=after, npcs=True), a('isDead', target='n0')]

    killer = [player(name='Zezima')]
    for duration in [0, 1, 2, 3, 6, 29, 30, 31, -5, INT_MIN, INT_MAX]:
        add(death(), players=killer, npcs=[npc(tables=table, deathAnim=[836, duration])], seeds=dict(random=duration & 15))
    for delay in [None, 0, 1, 2, 3, 4, 17, 25, -1, -40, INT_MAX, 'x', True]:
        config = {} if delay is None else dict(respawn_delay=delay)
        add(death(after=30), players=killer, npcs=[npc(tables=table, deathAnim=[836, 2], config=config)])
    add(death(), players=killer, npcs=[npc(tables=table, deathAnim=[836, 2], config=dict(respawn_delay=None))])
    for audio in [None, [1, 2, 3], [1, 2, 0], [0, 0, 511, 9], [1, 2], [], 'x', 7]:
        config = {} if audio is None else dict(combat_audio=audio)
        add(death(), players=killer, npcs=[npc(tables=table, deathAnim=[836, 2], config=config, deathGfx=None if audio is None else 86)])
    add(death(), players=killer, npcs=[npc(definition=False)])
    add(death(), players=killer, npcs=[npc(tables='null', deathAnim=[836, 2])])
    add(death(), players=killer, npcs=[npc(deathAnim=[836, 2])])
    for change in [dict(disableDrop=True), dict(zoneStart=False), dict(zoneHandle=True), dict(respawn=False), dict(safeZone=True),
                   dict(safeZone=True, safeRespawn=[3100, 3100, 1]), dict(spawn=[3215, 3215, 0]), dict(spawn=None), dict(location=[3212, 3210, 0], spawn=[3210, 3210, 0]),
                   dict(multi=True), dict(dropLocation=[3209, 3211, 0]), dict(originalId=77)]:
        add(death(), players=killer, npcs=[npc(MESSAGE_NPC, tables=table, deathAnim=[836, 2], **change)])
    for id in [1240, 1241, 1239, 1242]:
        add(death(), players=killer, npcs=[npc(id, tables=table, deathAnim=[836, 2])])
    # Who the death is credited to decides whether anything drops.
    crowd = [player(name='A', uid=7), player(name='B', uid=3, ironman=True), player(name='C', uid=21, ai=True)]
    for hits, last in [([('p0', 5), ('p1', 9)], 'p0'), ([('p0', 9), ('p1', 5)], 'p1'), ([('p1', 9)], 'p1'), ([('p1', 9), ('p1', 2)], 'p1'),
                       ([('p0', 5), ('p1', 5)], 'p1'), ([('p1', 5), ('p0', 5)], 'p1'), ([('n1', 30), ('p0', 1)], 'n1'), ([('n1', 30)], 'n1'),
                       ([('n1', 30)], None), ([], 'p0'), ([('p0', 9)], 'p2'), ([('p0', 9), ('p1', 3)], 'p2'), ([('p2', 9), ('p1', 3)], 'p0')]:
        steps = [hit(source, amount) for source, amount in hits] + [hit(last, 0, lethal=True), a('tick', count=4), a('tick', count=20, npcs=True)]
        add(steps, players=crowd, npcs=[npc(MESSAGE_NPC, tables=table, deathAnim=[836, 2]), npc(2)], seeds=dict(random=len(cases)))
    add([hit('p0', 9), a('set', player=0, online=False), hit('p0', 1, lethal=True), a('tick', count=4)], players=crowd,
        npcs=[npc(tables=table, deathAnim=[836, 2])])
    # A dying player: the task is the same, the player's own death handling is a service.
    add([hit('n0', 9, target='p0'), hit('n0', 3, lethal=True, target='p0'), a('isDead', target='p0'), a('tick', count=8), a('isDead', target='p0')],
        players=[player(deathAnim=[9055, 4]), player()], npcs=[npc()])
    add([hit('p1', 9, target='p0'), hit('p1', 3, lethal=True, target='p0'), a('tick', count=8)],
        players=[player(deathAnim=[9055, 4], safeZone=True, safeRespawn=[2000, 2000, 0]), player()], npcs=[npc()])
    add([a('startDeath', target='p0', killer=None), a('tick', count=8)], players=[player(deathAnim=[9055, 0]), player()], npcs=[npc()])
    # Deaths that do not run their course.
    add([a('startDeath', target='n0', killer=None), a('tick', count=4), a('tick', count=20, npcs=True)], players=killer,
        npcs=[npc(tables=table, deathAnim=[836, 2])])
    add([a('startDeath', target='n0', killer='n1'), a('tick', count=4)], players=killer, npcs=[npc(tables=table, deathAnim=[836, 2]), npc(2)])
    add([hit('p0', 3, lethal=True), hit('p0', 3, lethal=True), a('startDeath', target='n0', killer='p0'), a('tick', count=4),
         hit('p0', 3, lethal=True), a('tick', count=18, npcs=True), hit('p0', 3, lethal=True), a('tick', count=4)],
        players=killer, npcs=[npc(tables=table, deathAnim=[836, 2])])
    add([a('strong', target='n0', delay=3), hit('p0', 3, lethal=True), a('isDead', target='n0'), a('tick', count=8, npcs=True), a('isDead', target='n0'),
         hit('p0', 3, lethal=True)], players=killer, npcs=[npc(tables=table, deathAnim=[836, 2])])
    add([hit('p0', 3, lethal=True), a('tick', count=1), a('set', target='n0', active=False), a('tick', count=3), a('tick', count=3, npcs=True)],
        players=killer, npcs=[npc(tables=table, deathAnim=[836, 6])])
    add([hit('p0', 3, lethal=True), a('set', target='n0', disableDrop=True), a('tick', count=4)], players=killer,
        npcs=[npc(tables=table, deathAnim=[836, 2])])
    add([hit('p0', 3, lethal=True), a('set', target='n0', disableDrop=None), a('tick', count=4)], players=killer,
        npcs=[npc(tables=table, deathAnim=[836, 2], disableDrop=True)])
    # A behaviour that fails while the death finishes: Pulse.update stops the pulse again, so the death finishes twice.
    for behavior in [dict(failAt=1), dict(failAt=2), dict(failAt=1, add=[4151, 1])]:
        add([hit('p0', 3, lethal=True), a('tick', count=5), a('tick', count=20, npcs=True)], players=killer,
            npcs=[npc(MESSAGE_NPC, tables=table, deathAnim=[836, 2])], behavior=behavior)
    # Near the end of the tick counter.
    for tick in [INT_MAX - 3, INT_MAX - 60, INT_MIN + 2, -1, 0]:
        add(death(ticks=6, after=4), players=killer, npcs=[npc(tables=table, deathAnim=[836, 2])], tick=tick)
    # Two NPCs dying in one tick share the generators in pulse order.
    add([hit('p0', 3, lethal=True, target='n1'), hit('p0', 3, lethal=True), a('tick', count=5), a('tick', count=20, npcs=True)], players=killer,
        npcs=[npc(tables=table, deathAnim=[836, 2]), npc(2, tables=table, deathAnim=[836, 2])])
    add([hit('p0', 3, lethal=True), a('tick', count=1), hit('p0', 3, lethal=True, target='n1'), a('tick', count=6), a('tick', count=20, npcs=True)],
        players=killer, npcs=[npc(tables=table, deathAnim=[836, 3]), npc(2, tables=table, deathAnim=[836, 1])])
    # A whole kill with loot share, a listed NPC and every kind of drop.
    full = [main(1163, 1, 1, 1.0, True), main(536, 1, 1, 1.0, True), main(561, 5, 9, 1.0, True), main(4675, 1, 1, 1.0, True), main(995, 100, 2000, 1.0),
            charm(12158, 1, 2, 1.0), tertiary(6199, 1, 1, 1.0)]
    for seed in range(6):
        add([hit('p1', 40), hit('p0', 60), hit('p0', 1, lethal=True), a('tick', count=4), a('tick', count=20, npcs=True), a('set', tick=1104), a('groundPulse')],
            players=[share(lsp=5, name='Killer', boneCrusher=seed % 2 == 0), share(lsp=50, name='Mate', resizable=True, location=[3205, 3205, 0]),
                     player(name='Stranger', location=[3190, 3190, 0])],
            clans=[2], local=[0, 1, 2], npcs=[npc(MESSAGE_NPC, tables=full, deathAnim=[836, 2], multi=True, config=dict(respawn_delay=12, combat_audio=[10, 11, 12]))],
            prices={'1163': 20000, '561': 300, '995': 1, '4675': 90000}, seeds=dict(random=seed, tlr=seed * 31 + 1))
    return cases


def line(text, pattern):
    """One declaration line of an original source, as written."""
    match = re.search(pattern, text, re.M)
    if not match:
        raise RuntimeError(f'declaration not found: {pattern}')
    return match.group(0).strip()


def markers():
    def source(name):
        return c.read(SERVER / name)

    entity = source('core/game/node/entity/Entity.java')
    npc = source('core/game/node/entity/npc/NPC.java')
    impact = source('core/game/node/entity/combat/ImpactHandler.java')
    item = source('core/game/node/item/Item.java')
    item_definition = source('core/cache/def/impl/ItemDefinition.java')
    npc_definition = source('core/cache/def/impl/NPCDefinition.java')
    location = source('core/game/world/map/Location.kt')
    api = source('core/api/ContentAPI.kt')
    animator = source('core/game/node/entity/impl/Animator.java')
    member = 'fun getDistance(other: Location): Double = getDistance(this, other)'
    if member not in location:
        raise RuntimeError('Location.getDistance(other) differs')
    if not re.search(r'\bPRAYER = 5\b', source('core/game/node/entity/skill/Skills.java')):
        raise RuntimeError('Skills.PRAYER differs')
    if '@JvmOverloads\nfun playGlobalAudio(location: Location, id: Int,' not in api:
        raise RuntimeError('playGlobalAudio differs')
    return {
        'ORIGINAL_ENTITY_COMMENCE_DEATH': c.method(entity, 'public void commenceDeath(Entity killer)'),
        'ORIGINAL_ENTITY_FINALIZE_DEATH': c.method(entity, 'public void finalizeDeath(Entity killer)'),
        'ORIGINAL_ENTITY_START_DEATH': c.method(entity, 'public void startDeath(Entity killer)'),
        'ORIGINAL_NPC_TICK': c.method(npc, 'public void tick()'),
        'ORIGINAL_NPC_ON_RESPAWN': c.method(npc, 'protected void onRespawn()'),
        'ORIGINAL_NPC_FINALIZE_DEATH': c.method(npc, 'public void finalizeDeath(Entity killer)'),
        'ORIGINAL_NPC_COMMENCE_DEATH': c.method(npc, 'public void commenceDeath(Entity killer)'),
        'ORIGINAL_NPC_HANDLE_DROPS': c.method(npc, 'public void handleDrops(Player p, Entity killer)'),
        'ORIGINAL_NPC_GET_AUDIO': c.method(npc, 'public Audio getAudio(int index)'),
        'ORIGINAL_NPC_GET_DROP_LOCATION': c.method(npc, 'public Location getDropLocation()'),
        'ORIGINAL_NPC_IS_RESPAWN': c.method(npc, 'public boolean isRespawn()'),
        'ORIGINAL_NPC_GET_RESPAWN_TICK': c.method(npc, 'public int getRespawnTick()'),
        'ORIGINAL_NPC_SET_RESPAWN_TICK': c.method(npc, 'public void setRespawnTick(int respawnTick)'),
        'ORIGINAL_HIT_LOG': c.method(impact, 'if (hit > 0) {'),
        'ORIGINAL_DEATH_BRANCH': c.method(impact, 'if (entity.getSkills().getLifepoints() < 1) {'),
        'ORIGINAL_GET_MOST_DAMAGE': c.method(impact, 'public Entity getMostDamageEntity(Entity killer)'),
        'ORIGINAL_GET_NPC_LOG': c.method(impact, 'public Map<Entity, Integer> getNpcImpactLog()'),
        'ORIGINAL_GET_PLAYER_LOG': c.method(impact, 'public Map<Integer, Integer> getPlayerImpactLog()'),
        'ORIGINAL_GET_DISABLED': c.method(impact, 'public int getDisabledTicks()'),
        'ORIGINAL_SET_DISABLED': c.method(impact, 'public void setDisabledTicks(int ticks)'),
        'ORIGINAL_RESET_A': line(animator, r'^\s*public static final Animation RESET_A = [^;]+;'),
        'ORIGINAL_RESET_G': line(animator, r'^\s*public static final Graphics RESET_G = [^;]+;'),
        'ORIGINAL_LOCATION_DISTANCE_MEMBER': member,
        'ORIGINAL_LOCATION_DISTANCE': c.method(location, 'fun getDistance(first: Location, second: Location)'),
        'ORIGINAL_CHUNK_ADD': c.method(source('core/game/world/map/RegionChunk.java'), 'public void add(GroundItem item)'),
        'ORIGINAL_CONSTRUCT_TYPE': line(source('core/game/world/update/flag/chunk/ItemUpdateFlag.java'), r'^\s*public static final int CONSTRUCT_TYPE = [^;]+;'),
        'ORIGINAL_ITEM_NOTE_CHANGE': c.method(item, 'public int getNoteChange()'),
        'ORIGINAL_ITEM_GET_ID': c.method(item, 'public int getId()'),
        'ORIGINAL_ITEM_GET_AMOUNT': c.method(item, 'public int getAmount()'),
        'ORIGINAL_ITEM_SET_AMOUNT': c.method(item, 'public void setAmount(int amount)'),
        'ORIGINAL_ITEM_GET_CHARGE': c.method(item, 'public int getCharge()'),
        'ORIGINAL_ITEM_HAS_PLUGIN': c.method(item, 'public boolean hasItemPlugin()'),
        'ORIGINAL_ITEM_GET_PLUGIN': c.method(item, 'public ItemPlugin getPlugin()'),
        'ORIGINAL_ITEM_GET_DEFINITION': c.method(item, 'public ItemDefinition getDefinition()'),
        'ORIGINAL_PLUGIN_DROP': line(source('core/game/node/item/ItemPlugin.java'), r'^\s*protected static final int DROP = [^;]+;'),
        'ORIGINAL_ITEMDEF_PLUGIN': c.method(item_definition, 'public ItemPlugin getItemPlugin()').replace('getItemPlugin()', 'itemPluginBody()', 1),
        'ORIGINAL_ITEMDEF_STACKABLE': c.method(item_definition, 'public boolean isStackable()'),
        'ORIGINAL_ITEMDEF_UNNOTED': c.method(item_definition, 'public boolean isUnnoted()'),
        'ORIGINAL_ITEMDEF_NOTE_ID': c.method(item_definition, 'public int getNoteId()'),
        'ORIGINAL_ITEMDEF_TRADEABLE': c.method(item_definition, 'public boolean isTradeable()'),
        'ORIGINAL_ITEMDEF_HAS_ACTION': c.method(item_definition, 'public boolean hasAction(String optionName)'),
        'ORIGINAL_ITEMDEF_DESTROY': c.method(item_definition, 'public boolean hasDestroyAction()'),
        'ORIGINAL_GET_CONFIGURATION': c.method(source('core/cache/def/Definition.java'), 'public <V> V getConfiguration(String key, V fail)'),
        'ORIGINAL_NPCDEF_DROP_TABLES': line(npc_definition, r'^\s*private NPCDropTables dropTables = [^;]+;'),
        'ORIGINAL_NPCDEF_GET_DROP_TABLES': c.method(npc_definition, 'public NPCDropTables getDropTables()'),
        'ORIGINAL_NPCDEF_SET_DROP_TABLES': c.method(npc_definition, 'public void setDropTables(NPCDropTables dropTables)'),
        'ORIGINAL_ANNOUNCE_IF_RARE': c.method(api, 'fun announceIfRare(player: Player, item: Item)'),
        'ORIGINAL_EXCEPTION_TO_STRING': c.method(api, 'fun exceptionToString ('),
        'ORIGINAL_LOG_ENUM': c.method(source('core/tools/SystemLogger.kt'), 'enum class Log {'),
        'ORIGINAL_FORMATTED_NUMBER': c.method(source('core/tools/StringUtils.java'), 'public static String getFormattedNumber(int amount)'),
        'ORIGINAL_GET_CLUE': c.method(source('content/global/activity/ttrail/ClueScrollPlugin.java'), 'public static Item getClue('),
    }


def constants(name, text, keys):
    """A Java holder for the named `const val` strings of an original Kotlin companion."""
    fields = ''
    for key in keys:
        match = re.search(rf'const val {key}\s*=\s*("[^"]*")', text)
        if not match:
            raise RuntimeError(f'{name}.{key} not found')
        fields += f'\tpublic static final String {key} = {match.group(1)};\n'
    return f'package core.game.system.config;\n\n/** Constant values taken from the original companion at generation time. */\npublic class {name} {{\n{fields}}}\n'


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)

    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libraries = [stdlib]
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))
        jar = RUN / name
        # Pinned by hash; this generator never downloads. Place the artifact here from another oracle's run directory.
        if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != pin['sha256']:
            raise RuntimeError(f'{jar} is missing or differs from its pin')
        libraries.append(jar)

    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    texts = dict(adapters.ADAPTERS)
    texts['core/game/system/config/NPCConfigParser.java'] = constants(
        'NPCConfigParser', c.read(SERVER / 'core/game/system/config/NPCConfigParser.kt'), ['RESPAWN_DELAY', 'COMBAT_AUDIO'])
    texts['core/game/system/config/ItemConfigParser.java'] = constants(
        'ItemConfigParser', c.read(SERVER / 'core/game/system/config/ItemConfigParser.kt'), ['TRADEABLE', 'RARE_ITEM'])
    runner = c.read(SERVER / 'core/game/world/PulseRunner.kt')
    texts['core/game/world/PulseRunner.kt'] = runner.replace('package core.game.world', 'package core.game.world\nimport core.game.world.PulseClock as System', 1)
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(texts.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'adapters.py')
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')

    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx768m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, libraries)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])

    cases = requests()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '--add-opens', 'java.base/java.lang=ALL-UNNAMED',
               '--add-opens', 'java.base/java.util=ALL-UNNAMED', '-cp', classpath, 'oracle.Support']
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=600)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')

    steps = [(action, step) for row in rows for action, step in zip(row['actions'], row['expected'])]
    events, errors, operations = {}, {}, {}
    for action, step in steps:
        operations[action['op']] = operations.get(action['op'], 0) + 1
        if step['error'] is not None:
            errors[step['error']] = errors.get(step['error'], 0) + 1
        for event in step['events']:
            events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(
        actions=len(steps),
        operations=dict(sorted(operations.items())),
        serviceCalls=sum(events.values()),
        events=dict(sorted(events.items())),
        errors=dict(sorted(errors.items())),
        groundItems=sum(len(row['expected'][-1]['state']['grounds']) for row in rows),
        deathsFinished=events.get('onDeathFinished', 0),
        ticks=events.get('tick', 0),
    )
    for needed in ['flagged', 'packet', 'pluginRemove', 'news', 'setLsp', 'xp', 'aiAdd', 'botDrop', 'stackTrace', 'transform', 'teleport', 'onRespawn',
                   'playerFinalizeDeath', 'globalAudio', 'clear']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', scope=SCOPE)
    summary = (f'{len(rows)} original NPC death and drop cases ({coverage["actions"]} actions, {coverage["serviceCalls"]} service calls, '
               f'{coverage["groundItems"]} ground items, {coverage["deathsFinished"]} finished deaths)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('NPC death fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
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
