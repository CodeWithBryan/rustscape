"""Designed corpus for the persisted state of the systems behind save::Host.

Every case is marked "state": true. Each system gets its fresh state, its
fields at their boundaries, its maps' iteration order under several insertion
orders, missing, extra and wrong-typed keys, and round trips through the
unmodified PlayerSaver/PlayerSaveParser. The first case family is the one that
matters most: a brand-new character's whole file, written and loaded back.
"""
import json, random

QUEST_NAMES = []  # filled by state.py from the pinned Quests.kt


def op(_op, **args):
    return dict(op=_op, **args)


def case(label, actions, full=False, **world):
    return dict(state=True, label=label, world=world, actions=actions, full=full)


def text(value):
    return json.dumps(value, separators=(',', ':'))


# A registry shaped like the pinned content's: many quests over a wide index
# range, so the per-player map resizes and several indices share buckets.
def quest_registry(count=60, seed=1):
    rng = random.Random(seed)
    ordinals = rng.sample(range(len(QUEST_NAMES)), count)
    rows, used = [], set()
    for n, ordinal in enumerate(ordinals):
        index = rng.choice([n + 1, n * 16 + 3, rng.randrange(0, 300), 64 + n])
        used.add(index)
        rows.append([ordinal, index, index - 1, rng.choice([0, 1, 2, 3, 5])])
    return rows


SONGS = [[62, 'Newbie Melody', 0], [76, 'Harmony', 7], [100, 'a', 16], [101, 'b', 32], [102, 'c', 1], [103, 'd', 48], [104, 'e', 17], [105, 'f', 64], [106, 'g', 33],
         [107, 'h', 7], [108, ' ', 9], [109, 'i', 300], [110, 'j', -1], [111, 'k', 2147483647]]
SPELLS = [[0, 1, True, 0], [0, 2, False, 0], [1, 20, True, 1], [2, 7, True, 0], [0, 99, True, 2]]
REWARDS = [11136, 11137, 11756, 11753, 13560, 11185, 14577, 14580, 14571, 14574, 14631, 14633, 11141, 14635]
EMPTY_DIARY = {'startedLevels': [False] * 3, 'completedLevels': [[False] * 25] * 3, 'rewardedLevels': [False] * 3}
DIARY_KEYS = ['KARAMJA', 'VARROCK', 'LUMBRIDGE', 'FALADOR', 'FREMENNIK', 'SEERS_VILLAGE']
POUCH_IDS = [5509, 5510, 5512, 5514]


def whole_profile(label, world, setup, full=True):
    """Write the whole file, load it into a new character, write it again."""
    return case(label, setup + [op('populate'), op('profileParse'), op('contentHooks'), op('profileSave'), op('newPlayer'), op('profileParse'), op('contentHooks'), op('populate'), op('profileSave')], full=full, **world)


def fresh_cases():
    registries = [
        ('no content registered', {}),
        ('quests and songs registered', dict(quests=quest_registry(), songs=SONGS, spells=SPELLS)),
        ('two quests', dict(quests=[[3, 5, 4, 2], [1, 9, 8, 1]])),
        ('content hooks', dict(quests=quest_registry(20, 7), hooks=[['slayer', '{"taskStreak":"0"}'], ['barCrawl', '{"started":false,"bars":[]}']])),
    ]
    out = []
    for name, world in registries:
        out.append(whole_profile('fresh character: ' + name, world, []))
        out.append(case('fresh character state: ' + name, [op('diary.table'), op('quest.totals'), op('music.query', id=62), op('grave.get')], full=True, **world))
    return out


def quest_cases():
    out = []
    case = lambda label, actions, **world: globals()['case'](label, actions, questNames=QUEST_NAMES, **world)
    reg = quest_registry()
    # Iteration order of the stage map under several registration sets.
    for seed in range(8):
        r = quest_registry(random.Random(seed).choice([1, 5, 12, 13, 40, 90]), seed)
        out.append(case(f'quests: registration {seed}', [op('save', name='saveQuests')], quests=r))
    # Collisions in one bucket, keep insertion order; a resize splits them.
    for order in [[0, 16, 32, 48], [48, 32, 16, 0], [16, 0, 48, 32]]:
        rows = [[i, idx, 0, 1] for i, idx in enumerate(order)]
        out.append(case(f'quests: shared bucket {order}', [op('save', name='saveQuests'), op('quest.put', index=64, stage=5), op('quest.put', index=1, stage=2), op('save', name='saveQuests')], quests=rows))
    out.append(case('quests: duplicate index', [op('save', name='saveQuests'), op('quest.sync')], quests=[[0, 5, 0, 1], [1, 5, 0, 2], [2, -1, 0, 3]]))
    # Parse: values, missing and wrong-typed keys, unregistered stage keys.
    texts = [
        {'points': '7', 'questStages': [{'questId': '5', 'questStage': '100'}, {'questId': '9', 'questStage': '50'}]},
        {'points': 7, 'questStages': [{'questId': 5, 'questStage': 100}]},
        {'points': '2147483647', 'questStages': []},
        {'points': '-2147483648', 'questStages': [{'questId': '1000', 'questStage': '-1'}]},
        {'points': '2147483648', 'questStages': []},
        {'points': 'x', 'questStages': []},
        {'questStages': []},
        {'points': None, 'questStages': []},
        {'points': '1'},
        {'points': '1', 'questStages': None},
        {'points': '1', 'questStages': {}},
        {'points': '1', 'questStages': [None]},
        {'points': '1', 'questStages': ['a']},
        {'points': '1', 'questStages': [{'questStage': '1'}]},
        {'points': '1', 'questStages': [{'questId': '5'}]},
        {'points': '1', 'questStages': [{'questId': '5', 'questStage': '1.5'}]},
        {'points': '1', 'questStages': [{'questId': '5', 'questStage': 1.5}]},
        {'points': '1', 'questStages': [{'questId': '5', 'questStage': True}]},
        {'points': '1', 'questStages': [{'questId': ' 5', 'questStage': '1'}]},
        {'points': '1', 'questStages': [{'questId': '+5', 'questStage': '100'}, {'questId': '9', 'questStage': '100'}]},
        {'points': '1', 'questStages': [{'questId': '5', 'questStage': '100'}, {'questId': '5', 'questStage': '3'}], 'extra': 1},
        {'points': [1], 'questStages': []},
    ]
    small = [[3, 5, 4, 2], [1, 9, 8, 1], [7, 21, 20, 4]]
    for i, t in enumerate(texts):
        out.append(case(f'quests: parse {i}', [op('quest.parse', text=text(t)), op('save', name='saveQuests')], quests=small))
    # Points recomputed by syncPoints after parse, with point totals that wrap.
    out.append(case('quests: point wrap', [op('quest.parse', text=text({'points': '0', 'questStages': [{'questId': '5', 'questStage': '100'}, {'questId': '9', 'questStage': '100'}]})), op('quest.totals')], quests=[[3, 5, 4, 2147483647], [1, 9, 8, 1]]))
    # A quest registered after the character was built has no map entry.
    out.append(case('quests: late registration', [op('registry', world={'quests': [[11, 40, 39, 1]]}), op('quest.getStage', quest=11), op('quest.hasStarted', quest=11), op('quest.isComplete', quest=11), op('quest.parse', text=text({'points': '3', 'questStages': []})), op('save', name='saveQuests'),
        op('quest.parse', text=text({'points': '3', 'questStages': [{'questId': '40', 'questStage': '100'}]})), op('save', name='saveQuests')], quests=small))
    # Stage and point mutators.
    ops = [op('quest.stage', quest=3, stage=10), op('quest.stage', quest=3, stage=10), op('quest.stage', quest=3, stage=5), op('quest.stage', quest=3, stage=5, any=True), op('quest.stage', quest=1, stage=100),
           op('quest.points', value=2147483647), op('quest.points', value=1), op('quest.points', value=5, dock=True), op('quest.sync'), op('quest.getStage', quest=3), op('quest.getStage', quest=0),
           op('quest.isComplete', quest=1), op('quest.isComplete', quest=0), op('quest.hasStarted', quest=3), op('quest.hasStarted', quest=0), op('quest.totals'), op('quest.stage', quest=0, stage=1), op('save', name='saveQuests')]
    out.append(case('quests: stage and points', ops, quests=small))
    # Whole-file round trips.
    rng = random.Random(41)
    for n in range(6):
        stages = [op('quest.put', index=row[1], stage=rng.choice([0, 1, 50, 100, -5, 2147483647])) for row in reg if rng.random() < 0.5]
        out.append(case(f'quests: round trip {n}', stages + [op('save', name='saveQuests'), op('populate'), op('profileParse'), op('contentHooks'), op('profileSave'), op('newPlayer'), op('profileParse'), op('save', name='saveQuests')], quests=reg))
    return out


def music_cases():
    out = []
    orders = [[62, 76, 100, 101, 102], [101, 100, 76, 62, 102], [100, 101, 103, 106, 104], [104, 106, 103, 101, 100], [109, 110, 111, 62], [76, 107], [107, 76]]
    for o in orders:
        out.append(case(f'music: order {o}', [op('music.put', id=i) for i in o] + [op('save', name='saveMusicPlayer'), op('music.query', id=o[0]), op('music.query', id=999)], songs=SONGS))
    # More than 12 entries resize the table; clear keeps the larger table.
    many = [[1000 + i, 's', i * 16 + (i % 3)] for i in range(40)]
    out.append(case('music: resize and clear', [op('music.put', id=1000 + i) for i in range(40)] + [op('save', name='saveMusicPlayer'), op('music.clear'), op('music.put', id=1032), op('music.put', id=1000), op('music.put', id=1016), op('save', name='saveMusicPlayer'), op('music.query', id=1000)], songs=many))
    big = [[2000 + i, 's', i] for i in range(205)]
    out.append(case('music: air guitar at 200', [op('music.put', id=2000 + i) for i in range(199)] + [op('music.query', id=2000), op('music.put', id=2199), op('music.query', id=2000)], songs=big))
    out.append(case('music: all songs', [op('music.query', id=62), op('music.put', id=62), op('music.query', id=62), op('music.put', id=76), op('music.query', id=76)], songs=SONGS[:2]))
    for i, t in enumerate([['62', '76'], ['76', '62', '76'], ['999'], [62], [None], ['x'], ['62', '2147483648'], [], ['107', '76'], ['110', '111']]):
        out.append(case(f'music: parse {i}', [op('music.put', id=100), op('parse', name='parseMusic', text=text({'unlockedMusic': t})), op('save', name='saveMusicPlayer')], songs=SONGS))
    for i, root in enumerate([{}, {'unlockedMusic': None}, {'unlockedMusic': {}}, {'unlockedMusic': '62'}]):
        out.append(case(f'music: parse root {i}', [op('parse', name='parseMusic', text=text(root))], songs=SONGS))
    return out


def emote_cases():
    out = []
    for i, ops in enumerate([[], [op('emote.add', ordinal=22)], [op('emote.add', ordinal=44), op('emote.add', ordinal=30), op('emote.add', ordinal=44)], [op('emote.remove', ordinal=3), op('emote.add', ordinal=3)],
                              [op('emote.clear'), op('emote.add', ordinal=21)], [op('emote.clear')], [op('emote.remove', ordinal=40), op('emote.add', ordinal=0)]]):
        out.append(case(f'emotes: list {i}', ops + [op('save', name='saveEmoteData'), op('emote.isUnlocked', ordinal=22)]))
    for i, t in enumerate([['22', '30'], ['30', '22', '30'], ['0', '44'], ['45'], ['-1'], [22], [None], ['x'], [], ['21']]):
        out.append(case(f'emotes: parse {i}', [op('emote.remove', ordinal=5), op('parse', name='parseEmoteManager', text=text({'emoteData': t})), op('save', name='saveEmoteData')]))
    for i, root in enumerate([{}, {'emoteData': None}, {'emoteData': {}}]):
        out.append(case(f'emotes: parse root {i}', [op('parse', name='parseEmoteManager', text=text(root))]))
    return out


def diary_cases():
    out = []
    def entry(key, **kw):
        d = json.loads(json.dumps(EMPTY_DIARY))
        d.update(kw)
        return {key: d}
    full_level = [True] * 25
    texts = [
        [entry(k) for k in DIARY_KEYS],
        [entry('KARAMJA', startedLevels=[True, False, True], completedLevels=[full_level, [True] + [False] * 24, []], rewardedLevels=[True])],
        [entry('karamja', startedLevels=[True])],
        [entry('SEERS_VILLAGE', startedLevels=[True, True, True])],
        [entry("Seers' Village", startedLevels=[True])],
        [entry('seers_village', rewardedLevels=[False, True])],
        [entry('Seers Village')],
        [entry('LUMBRIDGE', startedLevels=[True, True, True, True])],
        [entry('LUMBRIDGE', startedLevels=[True, True, True, 'x'])],
        [entry('LUMBRIDGE', startedLevels=[None])],
        [entry('LUMBRIDGE', startedLevels=['true'])],
        [entry('FALADOR', completedLevels=[[True] * 26])],
        [entry('FALADOR', completedLevels=[[True] * 25 + ['x']])],
        [entry('FALADOR', completedLevels=[[], [], [], []])],
        [entry('FALADOR', completedLevels=[[], [], [], [True]])],
        [entry('FALADOR', completedLevels=[None])],
        [entry('FALADOR', completedLevels=['x'])],
        [{'FREMENNIK': {'startedLevels': [True]}}],
        [{'FREMENNIK': {'completedLevels': [], 'rewardedLevels': []}}],
        [{'FREMENNIK': None}],
        [{'FREMENNIK': []}],
        [{}],
        [None],
        ['KARAMJA'],
        [dict(**entry('KARAMJA', startedLevels=[True]), **entry('VARROCK', startedLevels=[True]))],
        [dict(**entry('VARROCK', startedLevels=[True]), **entry('FALADOR', startedLevels=[True]), **entry('KARAMJA', startedLevels=[True]))],
        [entry('UNKNOWN', startedLevels=[True]), entry('VARROCK', rewardedLevels=[True, True, True])],
        [entry('VARROCK', startedLevels=[True]), entry('VARROCK', startedLevels=[False, True])],
    ]
    for i, t in enumerate(texts):
        out.append(case(f'diaries: parse {i}', [op('diary.parse', text=text(t)), op('save', name='saveAchievementData')]))
    for i, root in enumerate([{'achievementDiaries': None}, {'achievementDiaries': {}}, {'achievementDiaries': [entry('VARROCK', startedLevels=[True])]}]):
        out.append(case(f'diaries: parse root {i}', [op('parse', name='parseAchievements', text=text(root))]))
    # The static completedLevels list is shared: two players' parses accumulate.
    t = text([entry('KARAMJA', completedLevels=[[True, False], [True]])])
    out.append(case('diaries: static completed levels', [op('diary.parse', text=t), op('newPlayer'), op('diary.parse', text=t)]))
    out.append(case('diaries: raw flags save', [op('diary.raw', type=t_, started=[True, t_ % 2 == 0, False], rewarded=[t_ == 5, False, True], tasks=[[t_ % 3, t_ * 4, True], [2, 24, True]]) for t_ in range(6)] + [op('save', name='saveAchievementData')]))
    # resetRewards: the parser runs it when the file has no diaries.
    placements = [
        [('inventory', 0, 11136, 1), ('inventory', 5, 11137, 1), ('bank', 0, 13560, 1), ('equipment', 9, 11141, 1)],
        [('inventory', 0, 11136, 2), ('bank', 3, 11136, 1), ('secondary', 0, 14631, 3), ('equipment', 4, 14631, 1)],
        [('inventory', 0, 3, 1), ('bank', 0, 4, 9)],
        [],
    ]
    for i, items in enumerate(placements):
        for secondary in [False, True]:
            ops = [op('item', container=c, slot=s, id=id_, amount=a) for c, s, id_, a in items] + [op('secondary', value=secondary)]
            out.append(case(f'diaries: reset rewards {i} secondary={secondary}', ops + [op('diary.reset'), op('parse', name='parseAchievements', text='{}')]))
    out.append(case('diaries: reset reward listener failure', [op('item', container='bank', slot=0, id=11756, amount=1), op('item', container='inventory', slot=1, id=11756, amount=1), op('failAt', tag='listener', at=2), op('diary.reset')]))
    return out


def spell_cases():
    out = []
    for i, spell in enumerate([[0, 1], [1, 20], [2, 7], [0, 99], None]):
        out.append(case(f'autocast: save {i}', [op('autocast', spell=spell), op('save', name='saveAutocast')], spells=SPELLS))
    for i, value in enumerate([{'book': '0', 'spellId': '1'}, {'book': '2', 'spellId': '7'}, {'book': '0', 'spellId': '2'}, {'book': '0', 'spellId': '5'}, {'book': '3', 'spellId': '1'}, {'book': '-1', 'spellId': '1'},
                               {'book': 0, 'spellId': '1'}, {'spellId': '1'}, {'book': '0'}, {'book': 'x', 'spellId': '1'}, None, [], {}]):
        root = {'autocastSpell': value} if value is not None else {}
        out.append(case(f'autocast: parse {i}', [op('parse', name='parseAutocastSpell', text=text(root)), op('save', name='saveAutocast')], spells=SPELLS))
    for i, value in enumerate(['0', '12', '13', '-1', 'x', 3, None, '07']):
        root = {'grave_type': value} if value is not None else {}
        out.append(case(f'grave: parse {i}', [op('parse', name='parseGrave', text=text(root)), op('grave.get'), op('save', name='saveAttributes')]))
    out.append(case('grave: set and read', [op('grave', ordinal=12), op('grave.get'), op('attr', key='/save:gravetype', kind='str', value='x'), op('grave.get'), op('attr', key='gravetype', kind='int', value=13), op('grave.get'),
        op('attr', key='gravetype', kind='long', value=1), op('grave.get'), op('attr', key='gravetype', kind='long', value=4294967300), op('grave.get'), op('attr', key='gravetype', kind='bool', value=True), op('grave.get'), op('failAt', tag='event'), op('grave', ordinal=4), op('grave.get')]))
    # A spell registered in another book than its own is saved under its own book and cannot be found again.
    out.append(whole_profile('autocast: spell of another book', dict(spells=SPELLS), [op('autocast', spell=[2, 7])], full=False))
    out.append(case('grave: whole file', [op('grave', ordinal=7), op('populate'), op('profileParse'), op('contentHooks'), op('profileSave'), op('newPlayer'), op('profileParse'), op('grave.get')]))
    return out


def pouch_cases():
    out = []
    items = [[1, False], [2, True], [3, False]]
    sets = [
        [op('pouch.set', id=5510, items=[[1, 1], [1, 1]])],
        [op('pouch.set', id=5514, charges=0, cap=0, remake=True)],
        [op('pouch.set', id=5509, items=[[2, 5], [2, 7], [1, 2]])],
        [op('pouch.set', id=5512, charges=-1, cap=2, remake=True, items=[[1, 5]])],
        [op('pouch.set', id=5510, charges=2147483647, cap=-1, remake=True)],
    ]
    for i, s in enumerate(sets):
        out.append(case(f'pouches: save {i}', s + [op('save', name='savePouches')], items=items))
    def pouch(id_, charges='3', cap='3', container=None, **extra):
        p = {'id': id_, 'charges': charges, 'currentCap': cap, 'container': container if container is not None else []}
        p.update(extra)
        return p
    texts = [
        [pouch('5510', '100', '4', [{'itemId': '1', 'amount': '1'}, {'itemId': '1', 'amount': '1'}])],
        [pouch(5512, 9, 9, [{'itemId': 2, 'amount': 20}])],
        [pouch('5509', '0', '0', [{'itemId': '1', 'amount': '1'}])],
        [pouch('5514', '1', '2', [{'itemId': '1', 'amount': '3'}])],
        [pouch('5514', '1', '-1')],
        [pouch('5511', '1', '2'), pouch('5509', '9', '9')],
        [pouch('5509', '9', '9'), pouch('9999', '1', '1'), pouch('5510', '1', '1')],
        [{'charges': '1', 'currentCap': '1', 'container': []}],
        [pouch('5509', None, '1')],
        [pouch('5509', 'x', '1')],
        [pouch('5509', '1', '2.0')],
        [pouch('5509', '2', '2', [{'amount': '1'}, {'itemId': None, 'amount': '1'}, {'itemId': '2', 'amount': '3'}])],
        [pouch('5509', '2', '2', [{'itemId': '2'}])],
        [pouch('5509', '2', '2', [None])],
        [pouch('5509', '2', '2', ['x'])],
        [{'id': '5509', 'charges': '2', 'currentCap': '2'}],
        [{'id': '5509', 'charges': '2', 'currentCap': '2', 'container': {}}],
        [None],
        ['5509'],
        [pouch(5509.0)],
        [pouch('5509', '2', '2', [{'itemId': '1', 'amount': '-4'}, {'itemId': '1', 'amount': '0'}, {'itemId': '0', 'amount': '1'}])],
        [pouch(p, str(c), str(c), [{'itemId': '1', 'amount': '1'}] * c) for p, c in zip(POUCH_IDS, [3, 6, 9, 12])],
    ]
    for i, t in enumerate(texts):
        out.append(case(f'pouches: parse {i}', [op('pouch.parse', text=text(t)), op('save', name='savePouches')], items=items))
    for i, root in enumerate([{}, {'pouches': None}, {'pouches': {}}, {'pouches': [pouch('5510', '7', '5')]}]):
        out.append(case(f'pouches: parse root {i}', [op('parse', name='parsePouches', text=text(root))], items=items))
    return out


def poh_cases():
    out = []
    for i, ops in enumerate([
        [op('poh.add', family=0, ids=[5, 3, 5, 1])],
        [op('poh.add', family=9, ids=[100, -1, 2147483647]), op('poh.add', family=3, ids=[7])],
        [op('poh.add', family=1, ids=[1, 2, 3]), op('poh.withdraw', family=1, id=2), op('poh.withdraw', family=1, id=9), op('poh.add', family=1, ids=[2])],
        [op('poh.add', family=2, ids=list(range(30)))] + [op('poh.page', family=2, total=30, size=10)] * 4,
        [op('poh.page', family=4, total=-1, size=0), op('poh.page', family=5, total=2147483647, size=2147483647), op('poh.page', family=5, total=1, size=-1)],
        [op('poh.add', family=6, ids=[1]), op('poh.clear')],
    ]):
        out.append(case(f'poh: save {i}', ops + [op('save', name='savePOHStorageRoom')]))
    reads = [
        {'containers': {'fancy_dress': {'items': [3, 1, 3]}, 'toy_box': {'items': []}}},
        {'containers': {'FANCY_DRESS': {'items': [3]}}},
        {'containers': {'cape_rack': {'items': [1.9, -1.5, 1e300, 4294967297, -2147483649]}}},
        {'containers': {'cape_rack': {'items': [1, None]}}},
        {'containers': {'cape_rack': {'items': [1, '2']}}},
        {'containers': {'cape_rack': {'items': {}}}},
        {'containers': {'cape_rack': {}}},
        {'containers': {'cape_rack': []}},
        {'containers': []},
        {'containers': None},
        {},
        {'containers': {'bookcase': {'items': [9]}, 'unknown': {'items': [1]}}},
    ]
    for i, r in enumerate(reads):
        pre = [op('poh.add', family=f, ids=[50 + f]) for f in range(10)] + [op('poh.add', family=7, ids=list(range(20))), op('poh.page', family=7, total=20, size=5)]
        out.append(case(f'poh: read {i}', pre + [op('poh.read', text=text(r)), op('save', name='savePOHStorageRoom')]))
    for i, root in enumerate([{'pohstorage': None}, {'pohstorage': []}, {'pohstorage': {'containers': {'armour_case': {'items': [8]}}}}, {}]):
        out.append(case(f'poh: parse root {i}', [op('parse', name='parsePOHStorable', text=text(root))]))
    return out


def house_familiar_cases():
    out = []
    houses = [
        dict(location=0, style=0, crest=16, servant=None, rooms=[]),
        dict(location=6, style=5, crest=0, servant=dict(type=5, uses=7, item=[1, 3], greet=True),
             rooms=[dict(z=0, x=4, y=3, properties=12, rotation=2, hotspots=[-1, 3, 0, -1]), dict(z=3, x=7, y=7, properties=1, rotation=0, hotspots=[]), dict(z=1, x=0, y=7, properties=2, rotation=3, hotspots=[2])]),
        dict(location=1, style=2, crest=16, servant=dict(type=0, uses=0, item=None, greet=False), rooms=[dict(z=0, x=0, y=0, properties=0, rotation=1, hotspots=[-1])]),
    ]
    for i, h in enumerate(houses):
        out.append(case(f'house: save {i}', [op('house', **h), op('save', name='saveHouseData'), op('parse', name='parseHouse', text=text({'houseData': {'location': '1'}}))]))
    pets = [[[7581, [['0.0', '100.0'], ['5.5', '0.1']]], [12482, []], [8, [['1.0E10', '-0.0']]]], [[16, [['1.0', '2.0']]], [0, []], [32, []], [48, [['3.0', '4.0']]]]]
    familiars = [None, dict(pet=7581), dict(originalId=6806, ticks=100, specialPoints=60, lifepoints=27), dict(originalId=6873, ticks=0, specialPoints=0, lifepoints=-1, burden=[[0, 1, 5, 1000], [29, 2, 1, 7]]),
                 dict(originalId=6873, ticks=0, specialPoints=0, lifepoints=0, burden=[])]
    for i, p in enumerate(pets):
        for j, f in enumerate(familiars):
            out.append(case(f'familiars: save {i}/{j}', [op('pets', pets=p), op('familiar', familiar=f), op('save', name='saveFamiliarManager'), op('parse', name='parseFamiliars', text=text({'familiarManager': {'petDetails': {}}}))]))
    return out


def populated_round_trips():
    """Random populated characters written whole, loaded into a new character and written again."""
    out = []
    rng = random.Random(530)
    reg = quest_registry(40, 9)
    for n in range(30):
        ops = []
        for row in reg:
            if rng.random() < 0.3:
                ops.append(op('quest.put', index=row[1], stage=rng.choice([0, 10, 100])))
        ops += [op('music.put', id=s[0]) for s in rng.sample(SONGS, rng.randrange(0, 8)) if s[1] != ' ']
        ops += [op('emote.add', ordinal=rng.randrange(22, 45)) for _ in range(rng.randrange(0, 4))]
        ops += [op('diary.raw', type=rng.randrange(6), started=[rng.random() < 0.5 for _ in range(3)], rewarded=[rng.random() < 0.3 for _ in range(3)], tasks=[[rng.randrange(3), rng.randrange(25), True] for _ in range(rng.randrange(0, 5))]) for _ in range(rng.randrange(0, 3))]
        if rng.random() < 0.5:
            ops.append(op('autocast', spell=rng.choice([[0, 1], [1, 20]])))
        if rng.random() < 0.5:
            ops.append(op('grave', ordinal=rng.randrange(13)))
        ops += [op('pouch.set', id=rng.choice(POUCH_IDS), charges=rng.randrange(-5, 300), items=[[rng.choice([1, 2]), rng.randrange(1, 4)]]) for _ in range(rng.randrange(0, 3))]
        ops += [op('poh.add', family=rng.randrange(10), ids=[rng.randrange(0, 20000) for _ in range(rng.randrange(1, 5))]) for _ in range(rng.randrange(0, 4))]
        if rng.random() < 0.3:
            ops.append(op('spellbook', value=rng.choice([192, 193, 430])))
        out.append(whole_profile(f'populated round trip {n}', dict(quests=reg, songs=SONGS, spells=SPELLS, items=[[1, False], [2, True]]), ops, full=False))
    return out


def failure_cases():
    """Failures raised by the services that stay outside native state, inside whole saves and loads."""
    out = []
    world = dict(quests=quest_registry(10, 3), songs=SONGS, hooks=[['hook', '{"a":"1"}']])
    out.append(case('whole file: hook save failure', [op('failAt', tag='hook.save'), op('populate')], **world))
    out.append(case('whole file: house parse failure', [op('populate'), op('saveParsed', value=True), op('profileSave'), op('newPlayer'), op('failAt', tag='house.parse'), op('profileParse')], **world))
    out.append(case('whole file: familiar parse failure', [op('music.put', id=62), op('populate'), op('saveParsed', value=True), op('profileSave'), op('newPlayer'), op('failAt', tag='familiar.parse'), op('profileParse')], **world))
    out.append(case('whole file: hook parse failure', [op('saveParsed', value=True), op('profileSave'), op('newPlayer'), op('failAt', tag='hook.parse'), op('profileParse'), op('contentHooks')], **world))
    # parseData stops at the first failing section and keeps what came before.
    base = {'quests': {'points': '0', 'questStages': [{'questId': '5', 'questStage': '100'}]}, 'unlockedMusic': ['62', '999'], 'emoteData': ['30']}
    out.append(case('whole file: music failure after quests', [op('populate'), op('saveParsed', value=True), op('profileSave'), op('newPlayer'), op('profileParse'),
        op('parse', name='parseMusic', text=text(base)), op('parse', name='parseQuests', text=text(base))], **world))
    return out


def seed_requests():
    return []


def requests(seed):
    return fresh_cases() + quest_cases() + music_cases() + emote_cases() + diary_cases() + spell_cases() + pouch_cases() + poh_cases() + house_familiar_cases() + populated_round_trips() + failure_cases()
