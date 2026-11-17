"""The designed corpus of the player managers oracle.

Each case lists nodes (players and NPCs with their containers, levels and
location), the actions to run and optionally hooks (actions run when an
event is recorded for the n-th time), failures (an event that throws on its
n-th call) and answers (scripted boolean answers). Groups name what a case
drives; generate.py counts them.
"""

TYPES = ['KARAMJA', 'VARROCK', 'LUMBRIDGE', 'FALADOR', 'FREMENNIK', 'SEERS_VILLAGE']
REWARDS = {
    'KARAMJA': [[11136, 11137], [11138, 11139], [11140, 11141]],
    'VARROCK': [[11756, 11753], [11757, 11754], [11758, 11755]],
    'LUMBRIDGE': [[13560, 11185], [13561, 11186], [13562, 11187]],
    'FALADOR': [[14577, 14580], [14578, 14581], [14579, 14582]],
    'FREMENNIK': [[14571, 14574], [14572, 14575], [14573, 14576]],
    'SEERS_VILLAGE': [[14631, 14633], [14631, 14634], [14631, 14635]],
}
NAMES = {}
for kind, levels in REWARDS.items():
    for level, (main, lamp) in enumerate(levels):
        NAMES[str(main)] = f'{kind.title()} reward {level + 1}'
        NAMES[str(lamp)] = 'Antique lamp'
NAMES['11185'] = 'Explorer\'s LAMP'
NAMES['14631'] = 'Enchanted headband'
SKILLS = list(range(-1, 25)) + [99]
ASSIST_SKILLS = [20, 12, 9, 22, 19, 6, 13, 7, 15]
TASK_COUNTS = {'KARAMJA': [10, 19, 10], 'VARROCK': [19, 21, 13], 'LUMBRIDGE': [21, 19, 12], 'FALADOR': [15, 12, 10],
               'FREMENNIK': [10, 11, 9], 'SEERS_VILLAGE': [12, 12, 11]}
BARS = ['BRONZE', 'BLURITE', 'IRON', 'SILVER', 'STEEL', 'GOLD', 'MITHRIL', 'ADAMANT', 'RUNITE']
INT_MAX = (1 << 31) - 1
INT_MIN = -(1 << 31)


def P(id, **kw):
    spec = dict(id=id, kind='player', name=f'player{id}', location=[3200 + id, 3200, 0])
    spec.update(kw)
    return spec


def N(id, **kw):
    spec = dict(id=id, kind='npc', location=[3300 + id, 3300, 0])
    spec.update(kw)
    return spec


def a(op, node=0, **kw):
    out = dict(op=op, node=node)
    out.update(kw)
    return out


def hook(event, actions, at=1):
    return dict(event=event, at=at, actions=actions)


def requests():
    cases = []

    def case(group, actions, nodes=None, **kw):
        row = dict(group=group, nodes=nodes if nodes is not None else [P(0), P(1), N(2)], actions=actions)
        row.update(kw)
        cases.append(row)

    skulls(case)
    hints(case)
    warnings(case)
    assists(case)
    diaries(case)
    return cases


def groups(cases):
    out = {}
    for row in cases:
        out[row['group']] = out.get(row['group'], 0) + 1
    return dict(sorted(out.items()))


# ------------------------------------------------------------------ SkullManager

GETTERS = ['skull.getLevel', 'skull.isWilderness', 'skull.isSkullCheckDisabled', 'skull.isWildernessDisabled',
           'skull.hasWildernessProtection', 'skull.isSkulled', 'skull.isDeepWilderness', 'skull.getPlayer']


def getters(node=0):
    return [a(op, node) for op in GETTERS]


def skulls(case):
    case('skull', [a('skullIcons'), a('threshold')] + [a('skullIconForId', id=i) for i in range(-3, 11)] + [a('skullIconForId', id=INT_MAX)])
    case('skull', getters())
    for op, values in [('skull.setWilderness', [True, False]), ('skull.setSkullCheckDisabled', [True, False]),
                       ('skull.setWildernessDisabled', [True, False]), ('skull.setSkulled', [True, False])]:
        steps = []
        for value in values:
            steps += [a(op, value=value)] + getters()
        case('skull', steps)
    levels = [INT_MIN, -1, 0, 1, 19, 20, 21, 47, 48, 49, 50, 56, INT_MAX]
    for enhanced in [False, True]:
        for start in [0, 49]:
            for level in levels:
                case('skull', [a('skull.setLevel', level=start), a('skull.setLevel', level=level)] + getters(), enhanced=enhanced)
        case('skull', [a('skull.setLevel', level=l) for l in [49, 48, 47, 48, 49, 50, 20, 21, 20, 0, 49, 47]] + getters(), enhanced=enhanced)
    for enhanced in [False, True]:
        for skulled in [False, True]:
            for deep in [True, False, True]:
                case('skull', [a('skull.setSkulled', value=skulled), a('skull.setDeepWilderness', value=deep)] + getters(), enhanced=enhanced)
            case('skull', [a('skull.setSkulled', value=skulled), a('skull.setDeepWilderness', value=True),
                           a('skull.setDeepWilderness', value=False), a('skull.setDeepWilderness', value=True)] + getters(),
                 enhanced=enhanced)
    case('skull', [a('deep', value=True), a('skull.setLevel', level=55), a('deep', value=False), a('skull.setLevel', level=30),
                   a('skull.setLevel', level=55), a('deep', value=True), a('skull.setLevel', level=40)] + getters())
    # The deep-wilderness risk: inventory and equipment, protect item, the threshold, wrapping.
    holdings = [
        dict(),
        dict(inventory=[[0, 995, 100000]]),
        dict(inventory=[[0, 1, 1], [1, 2, 1]], alchemy={'1': 100000, '2': 100000}),
        dict(inventory=[[0, 1, 1], [1, 2, 1]], alchemy={'1': 100000, '2': 99999}),
        dict(inventory=[[0, 1, 1], [1, 2, 1]], alchemy={'1': 100001, '2': 99999}),
        dict(inventory=[[0, 1, 2]], alchemy={'1': 50000}),
        dict(inventory=[[3, 1, 3]], equipment=[[4, 2, 1]], alchemy={'1': 40000, '2': 40000}),
        dict(equipment=[[3, 4151, 1], [4, 1127, 1], [7, 1079, 1]], alchemy={'4151': 72000, '1127': 38400, '1079': 38400}),
        dict(inventory=[[s, 554, 1000] for s in range(28)], alchemy={'554': 2}),
        dict(inventory=[[0, 1, INT_MAX], [1, 2, INT_MAX]], alchemy={'1': INT_MAX, '2': INT_MAX}),
        dict(inventory=[[0, 1, INT_MAX], [1, 2, INT_MAX], [2, 3, INT_MAX]], alchemy={'1': INT_MAX, '2': INT_MAX, '3': INT_MAX}),
        dict(inventory=[[0, 1, -5]], alchemy={'1': 30000}),
        dict(inventory=[[0, 1, 1]], equipment=[[0, 2, 1]], alchemy={'1': -7, '2': 0}),
        dict(inventory=[[27, 1, 1]], equipment=[[13, 2, 2]], alchemy={'1': 99999, '2': 100000}),
    ]
    for held in holdings:
        alchemy = held.pop('alchemy', {})
        for enhanced in [True, False]:
            case('skull', [a('skull.setLevel', level=49)] + getters() + [a('skull.updateDWSkullIcon'), a('skull.removeDWSkullIcon')],
                 [P(0, **held), P(1)], alchemy=alchemy, enhanced=enhanced)
    # Listener: added once, refreshed by container events, removed with the attribute.
    inv = dict(inventory=[[0, 1, 1]], equipment=[[0, 2, 1]])
    case('skull', [a('skull.setDeepWilderness', value=True), a('skull.listenerUpdate', container='inventory'),
                   a('invSet', container='inventory', slot=1, item=[3, 1]), a('skull.listenerRefresh', container='equipment'),
                   a('skull.listenerUpdate', container='bank'), a('skull.updateDWSkullIcon'),
                   a('skull.setDeepWilderness', value=True), a('skull.setDeepWilderness', value=False),
                   a('skull.listenerUpdate', container='inventory')],
         [P(0, **inv)], alchemy={'1': 60000, '2': 60000, '3': 100000}, enhanced=True)
    case('skull', [a('skull.setLevel', level=49), a('invSet', container='equipment', slot=3, item=[3, 1]), a('skull.listenerRefresh', container='inventory'),
                   a('skull.setLevel', level=48), a('skull.setLevel', level=47), a('skull.setLevel', level=49)],
         [P(0, **inv)], alchemy={'1': 60000, '2': 60000, '3': 100000}, enhanced=True)
    for value in [7, 'text', True, {'long': 12}, {'listener': True}]:
        for enhanced in [True, False]:
            case('skull', [a('setAttribute', key='deepwild-value-listener', value=value), a('skull.setDeepWilderness', value=True),
                           a('skull.setDeepWilderness', value=False), a('skull.removeDWSkullIcon')], [P(0, **inv)], enhanced=enhanced)
    case('skull', [a('setAttribute', key='deepwild-value-risk', value={'long': 5}), a('skull.removeDWSkullIcon'), a('skull.updateDWSkullIcon'),
                   a('skull.removeDWSkullIcon'), a('skull.removeDWSkullIcon')], [P(0, **inv)])
    # Reentrancy: a container listener added while the attribute is already set.
    case('skull', [a('skull.updateDWSkullIcon')], [P(0, **inv)], alchemy={'1': 200000},
         hooks=[hook('listenerAdd', [a('skull.listenerRefresh', container='inventory')])])
    case('skull', [a('skull.updateDWSkullIcon')], [P(0, **inv)], alchemy={'1': 200000},
         hooks=[hook('listenerAdd', [a('skull.removeDWSkullIcon')], at=2)])
    case('skull', [a('skull.updateDWSkullIcon')], [P(0, **inv)], alchemy={'1': 200000},
         hooks=[hook('toArray', [a('invSet', container='equipment', slot=0, item=[3, 1])])])
    case('skull', [a('skull.setDeepWilderness', value=True)], [P(0, **inv)], enhanced=True,
         hooks=[hook('skullIcon', [a('skull.setLevel', level=10)])])
    # checkSkull: every early return, the cause lists, the timer.
    players = [P(0), P(1), P(2), N(3)]
    case('skull', [a('skull.checkSkull', other=1)], players)
    case('skull', [a('skull.setWilderness', value=True), a('skull.checkSkull', other=3), a('skull.checkSkull', other=None),
                   a('skull.checkSkull', other=9)], players)
    case('skull', [a('skull.setWilderness', value=True), a('skull.setSkullCheckDisabled', value=True), a('skull.checkSkull', other=1)], players)
    for registry in [True, False]:
        case('skull', [a('skull.setWilderness', value=True), a('skull.checkSkull', other=1), a('skull.checkSkull', other=1),
                       a('skull.setWilderness', 1, value=True), a('skull.checkSkull', 1, other=0), a('skull.checkSkull', other=2),
                       a('skull.checkSkull', 2, other=0), a('skull.setWilderness', 2, value=True), a('skull.checkSkull', 2, other=0),
                       a('skull.checkSkull', 2, other=1)] + getters() + getters(2), players, registry=registry)
        case('skull', [a('skull.setWilderness', value=True), a('skull.checkSkull', other=0), a('skull.checkSkull', other=0),
                       a('skull.checkSkull', other=1)] + getters(), players, registry=registry)
    case('skull', [a('skull.setWilderness', value=True), a('skull.checkSkull', other=1), a('timerRun', timer=0), a('skull.checkSkull', other=1),
                   a('timerRemove', identifier='skulled'), a('skull.checkSkull', other=2), a('timerProcess'), a('skull.checkSkull', other=1),
                   a('timerProcess'), a('timerRemove', identifier='other'), a('timerRemove', identifier='skulled'),
                   a('timerRun', timer=2)] + getters(), players)
    case('skull', [a('skull.setWilderness', value=True), a('skull.setDeepWilderness', value=True), a('skull.checkSkull', other=1),
                   a('skull.setDeepWilderness', value=False), a('skull.checkSkull', other=1)] + getters(), players, enhanced=True)
    case('skull', [a('skull.setWilderness', value=True), a('skull.checkSkull', other=1), a('skull.reset'), a('skull.checkSkull', other=1),
                   a('skull.setSkullIcon', icon=3), a('skull.setSkullIcon', icon=-9)] + getters(), players)
    case('skull', [a('skull.setWilderness', value=True), a('skull.checkSkull', other=1)], players,
         hooks=[hook('skullIcon', [a('skull.checkSkull', other=2)])])
    case('skull', [a('skull.setWilderness', value=True), a('skull.checkSkull', other=1), a('skull.checkSkull', other=2)], players,
         hooks=[hook('timerInstance', [a('timerRemove', identifier='skulled')], at=2)])
    case('skull', [a('skull.setWilderness', value=True), a('skull.checkSkull', other=1), a('skull.checkSkull', other=2)], players,
         failures={'appearanceSync': 1})
    # Injected failures through each path.
    flows = {
        'check': ([a('skull.setWilderness', value=True), a('skull.checkSkull', other=1), a('skull.checkSkull', other=2), a('skull.reset')], False),
        'level': ([a('skull.setLevel', level=49), a('skull.setLevel', level=20), a('skull.setLevel', level=21), a('skull.setLevel', level=0)], True),
        'deep': ([a('skull.setDeepWilderness', value=True), a('skull.listenerRefresh', container='inventory'),
                  a('skull.setDeepWilderness', value=False)], True),
    }
    events = ['skullIcon', 'updateAppearance', 'appearanceSync', 'lockTeleport', 'unlockTeleport', 'timerInstance', 'getAttribute',
              'setAttribute', 'removeAttribute', 'listeners', 'listenerAdd', 'listenerRemove', 'toArray', 'alchemy']
    for name, (steps, enhanced) in flows.items():
        for event in events:
            for at in [1, 2]:
                case('skull', steps + getters(), [P(0, **inv), P(1), P(2)], alchemy={'1': 30000, '2': 80000}, enhanced=enhanced,
                     failures={event: at})


# ------------------------------------------------------------------ HintIconManager

def hints(case):
    nodes = [P(0), P(1, index=70000), N(2, index=255), N(3, index=-1)]
    case('hint', [a('hint.constants'), a('hint.freeSlot')] + [a('hint.getIcon', slot=s) for s in [0, 7, 8, -1]], nodes)
    targets = [{'node': 1}, {'node': 2}, {'node': 3}, {'node': 0}, {'location': [3222, 3218, 0]}, {'location': [70000, -1, 3]},
               {'location': None}, None]
    forms = ['register', 'registerHeight', 'registerArrow', 'registerModel', 'registerSlot', 'registerSlotHeight', 'registerFull']
    for target in targets:
        for form in forms:
            case('hint', [a('hint.' + form, target=target, arrow=2, model=40497, slot=3, height=150, type=7), a('hint.getIcon', slot=3),
                          a('hint.getIcon', slot=0), a('hint.freeSlot'), a('hint.remove', slot=3), a('hint.remove', slot=0)], nodes)
    for arrow in [-1, 0, 1, 2, 127, 255, 256, INT_MAX]:
        for model in [-1, 0, 40497, 65535, 70000]:
            case('hint', [a('hint.registerModel', target={'node': 2}, arrow=arrow, model=model),
                          a('hint.registerModel', target={'location': [3000, 3001, 1]}, arrow=arrow, model=model),
                          a('hint.clear')], nodes)
    for type in [-1, 0, 1, 2, 3, 9, 10, 11, 63, 64]:
        for target in [{'node': 1}, {'location': [3100, 3101, 0]}]:
            case('hint', [a('hint.registerFull', target=target, arrow=1, model=-1, slot=1, height=300, type=type), a('hint.remove', slot=1)], nodes)
    for slot in [-5, -1, 0, 6, 7, 8, 9, 100]:
        for form in ['registerSlot', 'registerSlotHeight', 'registerFull']:
            case('hint', [a('hint.' + form, target={'node': 2}, slot=slot, height=10, type=1), a('hint.freeSlot'),
                          a('hint.remove', slot=slot), a('hint.getIcon', slot=slot if 0 <= slot < 8 else 0)], nodes)
    for height in [-1, 0, 255, 256, 1000]:
        case('hint', [a('hint.registerHeight', target={'location': [3200, 3200, 0]}, height=height),
                      a('hint.registerHeight', target={'node': 1}, height=height), a('hint.clear')], nodes)
    # Filling every slot, the ninth refusal, removal in the middle, clear.
    fill = [a('hint.register', target={'node': 2 if i % 2 else 1}) for i in range(9)]
    case('hint', fill + [a('hint.freeSlot'), a('hint.remove', slot=4), a('hint.freeSlot'), a('hint.register', target={'node': 3}),
                         a('hint.clear'), a('hint.freeSlot'), a('hint.clear')], nodes)
    case('hint', fill[:3] + [a('hint.register', target=None), a('hint.registerSlot', target=None, slot=6), a('hint.freeSlot')], nodes)
    # The stored context is the one later removed: mutations show in the removal packet.
    for op, value in [('hint.setTargetType', 10), ('hint.setTargetType', 2), ('hint.setArrowId', 0), ('hint.setArrowId', -3),
                      ('hint.setArrowId', 9), ('hint.setHeight', 77)]:
        for target in [{'node': 1}, {'location': [3333, 3444, 2]}, {'location': None}]:
            case('hint', [a('hint.register', target=target), a(op, slot=0, value=value), a('hint.getIcon', slot=0), a('hint.remove', slot=0),
                          a('hint.registerHeight', target=target, height=20), a(op, slot=0, value=value), a('hint.clear')], nodes)
    case('hint', [a('hint.setHeight', slot=2, value=1), a('hint.getIcon', slot=9)], nodes)
    # Other players' managers, interleaving.
    case('hint', [a('hint.register', 0, target={'node': 1}), a('hint.register', 1, target={'node': 0}), a('hint.register', 0, target={'node': 2}),
                  a('hint.remove', 1, slot=0), a('hint.clear', 0), a('hint.freeSlot', 1)], nodes)
    # Reentrancy and failures.
    case('hint', [a('hint.register', target={'node': 1})], nodes, hooks=[hook('send', [a('hint.register', target={'node': 2})])])
    case('hint', [a('hint.register', target={'node': 1}), a('hint.register', target={'node': 2}), a('hint.register', target={'node': 3}),
                  a('hint.clear')], nodes, hooks=[hook('send', [a('hint.remove', slot=2)], at=4)])
    case('hint', [a('hint.register', target={'node': 1}), a('hint.register', target={'node': 2}), a('hint.clear')], nodes,
         hooks=[hook('send', [a('hint.register', target={'node': 3})], at=3)])
    flows = [
        [a('hint.register', target={'node': 1}), a('hint.registerHeight', target={'location': [1, 2, 0]}, height=5), a('hint.remove', slot=0),
         a('hint.registerFull', target={'node': 2}, slot=4, height=1, type=10, arrow=3, model=7)],
        [a('hint.register', target={'node': 1}), a('hint.register', target={'node': 2}), a('hint.register', target={'location': [5, 6, 0]}),
         a('hint.clear')],
    ]
    for steps in flows:
        for event in ['send', 'index', 'location']:
            for at in [1, 2, 3]:
                case('hint', steps + [a('hint.freeSlot')] + [a('hint.getIcon', slot=s) for s in range(5)], nodes, failures={event: at})


# ------------------------------------------------------------------ WarningMessages

def warnings(case):
    case('warn', [a('warn.open'), a('warn.open', 1)])
    case('warn', [a('warn.getMessage', index=i) for i in [0, 5, 0, -1, 5, INT_MAX, 19, 20, 21, 0]])
    case('warn', [a('warn.getMessage', index=i) for i in range(25)] + [a('warn.getMessage', index=24), a('warn.getMessage', index=0)])
    case('warn', [a('warn.toggle', index=3, other=0), a('warn.toggle', index=3, other=1), a('warn.toggle', index=4, other=None),
                  a('warn.toggle', index=4, other=2)])
    case('warn', [a('warn.toggle', 1, index=1, other=0), a('warn.getMessage', 1, index=1), a('warn.getMessage', 0, index=1)])
    for event, at in [('open', 1), ('varp', 1), ('message', 1), ('message', 2)]:
        case('warn', [a('warn.open'), a('warn.toggle', index=2, other=0), a('warn.open'), a('warn.toggle', index=2, other=0)],
             failures={event: at})
    case('warn', [a('warn.open')], hooks=[hook('open', [a('warn.getMessage', index=7)])])


# ------------------------------------------------------------------ AssistSession

def near(id, **kw):
    return P(id, location=[3200 + id, 3200, 0], **kw)


def assists(case):
    players = [near(0, levels={'20': 50, '12': 50, '13': 99}), near(1, levels={'20': 40, '12': 60, '13': 98}), near(2)]
    base = [a('assist.new', 0, partner=1)]
    case('assist', base + [a('assist.getSkillIndex', session=0, skill=s) for s in SKILLS], players)
    case('assist', [a('assist.newEmpty'), a('assist.getSkillIndex', session=0, skill=20), a('assist.getSkills', session=0),
                    a('assist.getTotalExperience', session=0), a('assist.getPlayer', session=0), a('assist.getPartener', session=0),
                    a('assist.isRestricted', session=0), a('assist.getExp', session=0), a('assist.open', session=0),
                    a('assist.refresh', session=0), a('assist.load', session=0), a('assist.pulse', session=0),
                    a('assist.addExperience', session=0, skill=20, exp=10), a('assist.getTimeLeft', session=0)], players)
    # The request module: ironman restrictions on either side, then the full opening.
    for answers in [[True], [False, True], [False, False]]:
        case('assist', [a('assist.module', 0, target=1), a('assist.getExtension', 0), a('assist.getExtension', 1)], players,
             answers={'ironmanRestriction': answers})
    for time in [0, 1, 999999999999, 1000000000000, 1000000000001, 1000086400000, 1000086400001]:
        case('assist', [a('assist.setGlobal', time=time, values=[1, 2.5, 30000, 0, 0, 0, 0, 0, 29999.5]), a('assist.module', 0, target=1)] +
             [a('assist.getTimeLeft', session=0), a('assist.isRestricted', session=0)], players)
    case('assist', [a('assist.setGlobal', values=[1, 2]), a('assist.module', 0, target=1)], players)
    case('assist', [a('assist.setGlobal', values=[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]), a('assist.module', 0, target=1), a('assist.getExp', session=0)],
         players)
    case('assist', [a('assist.module', 0, target=0)], players)
    case('assist', [a('assist.module', 0, target=1), a('assist.module', 1, target=0), a('assist.module', 2, target=0),
                    a('assist.getExtension', 0), a('assist.getExtension', 1), a('assist.getExtension', 2)], players)
    case('assist', [a('assist.extend', 0, partner=1), a('assist.getExtension', 0), a('assist.getExtension', 1), a('assist.open', session=0),
                    a('assist.getExtension', 1)], players)
    # Experience: the per-skill and total caps, restriction, unknown skills.
    for exp in [0, 1, 29999, 29999.99, 30000, 30001, -5, 1e9, 15000]:
        case('assist', base + [a('assist.addExperience', session=0, skill=20, exp=exp), a('assist.addExperience', session=0, skill=20, exp=exp),
                               a('assist.getExp', session=0), a('assist.isRestricted', session=0),
                               a('assist.getTotalExperience', session=0)], players)
    spread = [a('assist.addExperience', session=0, skill=s, exp=3500) for s in ASSIST_SKILLS]
    case('assist', base + spread + [a('assist.isRestricted', session=0), a('assist.addExperience', session=0, skill=7, exp=1)] +
         [a('assist.getExp', session=0)], players)
    case('assist', base + spread[:8] + [a('assist.addExperience', session=0, skill=15, exp=2000), a('assist.isRestricted', session=0),
                                        a('assist.addExperience', session=0, skill=15, exp=1000), a('assist.isRestricted', session=0),
                                        a('assist.refresh', session=0), a('assist.isRestricted', session=0)], players)
    case('assist', base + [a('assist.addExperience', session=0, skill=s, exp=10) for s in [0, 3, 99, -1]], players)
    case('assist', base + [a('assist.setExp', session=0, values=[30000, 0, 0, 0, 0, 0, 0, 0, 0]), a('assist.addExperience', session=0, skill=12, exp=1),
                           a('assist.isRestricted', session=0), a('assist.setExp', session=0, values=[40000, -20000, 0, 0, 0, 0, 0, 0, 0]),
                           a('assist.refresh', session=0), a('assist.getExp', session=0), a('assist.isRestricted', session=0),
                           a('assist.setExp', session=0, values=[1, 2]), a('assist.refresh', session=0), a('assist.getTotalExperience', session=0),
                           a('assist.setExp', session=0, values=[0.4, 0.6, 1e10, 0, 0, 0, 0, 0, 0, 7]), a('assist.refresh', session=0)],
         players)
    # The time left at each unit boundary.
    for offset in [-5000, 0, 999, 1000, 59999, 60000, 119999, 120000, 3599999, 3600000, 7199999, 7200000, 86400000, 10 ** 13]:
        case('assist', base + [a('assist.setGlobal', time=1000000000000 + offset), a('assist.load', session=0), a('assist.getTimeLeft', session=0),
                               a('assist.setExp', session=0, values=[30000, 0, 0, 0, 0, 0, 0, 0, 0]), a('assist.refresh', session=0)], players)
    # Buttons and toggles.
    case('assist', base + [a('assist.toggleButton', session=0, id=i) for i in [0, 3, 8, 3, 9, -1, 127]] +
         [a('assist.getSkills', session=0)] + [a('assist.isToggled', session=0, skill=s) for s in ASSIST_SKILLS + [0, 99]] +
         [a('assist.refresh', session=0)], players)
    # Translating experience.
    for toggled in [False, True]:
        for slot in [20, 12, 13, 9, 0]:
            for exp in [10.5, 29990, 30000, 45000]:
                steps = base + ([a('assist.toggleButton', session=0, id=i) for i in range(9)] if toggled else [])
                steps += [a('assist.translate', session=0, node=1, slot=slot, exp=exp, mod=0.5), a('assist.translate', session=0, node=0, slot=slot,
                                                                                                    exp=exp, mod=1), a('assist.getExp', session=0)]
                case('assist', steps, players)
    case('assist', base + [a('assist.toggleButton', session=0, id=0), a('assist.setExp', session=0, values=[29000, 0, 0, 0, 0, 0, 0, 0, 1000]),
                           a('assist.translate', session=0, node=1, slot=20, exp=5000, mod=2), a('assist.isRestricted', session=0),
                           a('assist.translate', session=0, node=1, slot=20, exp=5, mod=2)], players)
    for stored, exp, mod in [(29990.1, 10.3, 1), (29999.7, 0.35, 0.3), (12345.6, 17654.45, 1.1), (0.1, 29999.95, 2)]:
        case('assist', base + [a('assist.toggleButton', session=0, id=0), a('assist.setExp', session=0, values=[stored, 0, 0, 0, 0, 0, 0, 0, 0]),
                               a('assist.translate', session=0, node=1, slot=20, exp=exp, mod=mod), a('assist.getExp', session=0),
                               a('assist.getTotalExperience', session=0)], players)
    case('assist', base + [a('assist.toggleButton', session=0, id=0), a('assist.translate', session=0, node=1, slot=20, exp=100, mod=1)],
         [near(0, levels={'20': 40}), near(1, levels={'20': 50}), near(2)])
    # The pulse: distance, activity, the daily reset, kill.
    for location in [[3220, 3200, 0], [3221, 3200, 0], [3212, 3216, 0], [3213, 3216, 0], [3200, 3200, 1], [3200 + 65536, 3200, 0]]:
        case('assist', [a('assist.module', 0, target=1), a('move', 1, location=location), a('assist.pulse', session=0),
                        a('assist.pulse', session=0)], players)
    for who in [0, 1]:
        case('assist', [a('assist.module', 0, target=1), a('active', who, value=False), a('assist.pulse', session=0)], players)
    for now in [1000086399999, 1000086400000, 1000086400001]:
        case('assist', [a('assist.module', 0, target=1), a('assist.addExperience', session=0, skill=12, exp=500), a('clock', now=now),
                        a('assist.pulse', session=0), a('assist.getExp', session=0)], players)
    # Closing: through the manager, the event directly, then what the session still does.
    case('assist', [a('assist.module', 0, target=1), a('assist.addExperience', session=0, skill=12, exp=500), a('assist.close', 0),
                    a('assist.pulse', session=0), a('assist.close', 0), a('assist.getExtension', 0), a('assist.getExtension', 1)], players)
    case('assist', [a('assist.module', 0, target=1), a('assist.addExperience', session=0, skill=12, exp=500), a('assist.close', 0),
                    a('clock', now=1000090000000), a('assist.pulse', session=0), a('assist.addExperience', session=0, skill=12, exp=7)], players)
    case('assist', [a('assist.module', 0, target=1), a('assist.closeEvent', session=0, node=2), a('assist.pulse', session=0)], players)
    case('assist', [a('assist.module', 0, target=1), a('move', 1, location=[4000, 4000, 0]), a('assist.pulse', session=0),
                    a('assist.pulse', session=0)], players)
    case('assist', base + [a('assist.save', session=0), a('assist.addExperience', session=0, skill=20, exp=100), a('assist.reset', session=0),
                           a('assist.getExp', session=0), a('assist.addExperience', session=0, skill=20, exp=100)], players)
    case('assist', base + [a('assist.setPartener', session=0, node=2), a('assist.getPartener', session=0), a('assist.open', session=0),
                           a('assist.setPartener', session=0, node=None), a('assist.close', 0)], players)
    case('assist', base + [a('assist.toggleIcon', session=0, node=1, hide=True), a('assist.toggleIcon', session=0, node=2, hide=False)], players)
    # Reentrancy and failures in the opening and the close.
    case('assist', [a('assist.module', 0, target=1)], players, hooks=[hook('submit', [a('assist.close', 0)])])
    case('assist', [a('assist.module', 0, target=1)], players, hooks=[hook('open', [a('assist.close', 0)])])
    case('assist', [a('assist.module', 0, target=1), a('assist.close', 0)], players,
         hooks=[hook('restoreTabs', [a('assist.pulse', session=0)])])
    open_events = ['ironmanRestriction', 'face', 'getExtension', 'addExtension', 'lock', 'open', 'globalData', 'message', 'username', 'string',
                   'animate', 'graphics', 'audio', 'ifaceConfig', 'submit', 'varp', 'clock']
    for event in open_events:
        for at in [1, 2]:
            case('assist', [a('assist.module', 0, target=1), a('assist.getExtension', 1), a('assist.close', 0), a('assist.getExtension', 1)],
                 players, failures={event: at})
    close_events = ['close', 'componentClose', 'skillsRefresh', 'removeExtension', 'unlock', 'ifaceConfig', 'restoreTabs', 'message', 'username',
                    'globalData']
    for event in close_events:
        for at in [1, 2, 6]:
            case('assist', [a('assist.extend', 0, partner=1), a('assist.open', session=0), a('assist.close', 0), a('assist.pulse', session=0)],
                 players, hooks=[hook('submit', [a('clearCalls')])], failures={event: at})
    for event in ['location', 'isActive', 'clock', 'string', 'varp', 'close']:
        case('assist', [a('assist.module', 0, target=1), a('assist.pulse', session=0), a('assist.pulse', session=0)], players, failures={event: 3})
    for event in ['level', 'addExperience', 'string', 'varp']:
        case('assist', base + [a('assist.toggleButton', session=0, id=1), a('assist.translate', session=0, node=1, slot=12, exp=50, mod=1),
                               a('assist.getExp', session=0)], players, failures={event: 1})


# ------------------------------------------------------------------ Achievement diaries

def started(type, levels=(0, 1, 2)):
    return [a('ad.setLevelStarted', type=type, level=l) for l in levels]


def complete(type, level, upto=None):
    count = TASK_COUNTS[type][level] if upto is None else upto
    return [a('ad.setCompleted', type=type, level=level, index=i) for i in range(count)]


def diaries(case):
    nodes = [P(0), P(1, artificial=True), N(2)]
    case('diary', [a('diaryTable')] + [a('forChild', child=c) for c in range(-1, 35)] + [a('forChild', child=INT_MAX)], names=NAMES)
    for type in TYPES:
        case('diary', [a('diaryRewards', type=type, level=l) for l in [0, 1, 2, 3, -1]] + [a('diaryNpc', type=type, level=l) for l in [0, 2, 3]] +
             [a('diaryAchievements', type=type, level=l) for l in [0, 3]] + [a('hasRewardEquipment', type=type)], names=NAMES)
    for type in TYPES:
        for kind in ['inventory', 'bank', 'equipment']:
            for level in [0, 2]:
                for named in [True, False]:
                    reward, lamp = REWARDS[type][level]
                    case('diary', [a('hasRewardEquipment', type=type)], [P(0, **{kind: [[5, lamp, 1], [6, reward, 1]] if named else [[5, lamp, 1]]})],
                         names=NAMES if named else {})
    case('diary', [a('barOrdinal', bar=b) for b in BARS])
    case('diary', [a('diary.getDiarys'), a('diary.getPlayer')] + [a('diary.getDiary', type=t) for t in TYPES + [None]] +
         [a('diary.isComplete', type=t) for t in TYPES] + [a('diary.isComplete', type=None)])
    # Parsing saved diaries: the persisted state joined here.
    def saved(name, started=(True, False, False), completed=((True, False), (), ()), rewarded=(False, False, False)):
        return {name: {'startedLevels': list(started), 'completedLevels': [list(level) for level in completed], 'rewardedLevels': list(rewarded)}}
    parses = [
        [saved('Karamja'), saved('VARROCK', completed=((True,) * 14, (True, False), (False,)))],
        [saved('SEERS_VILLAGE'), saved('Seers_Village'), saved('seers_village', rewarded=(True, True, True))],
        [saved('LUMBRIDGE', started=(True, True, True, True))],
        [saved('FALADOR', completed=((), (), (), (True,)))],
        [saved('FREMENNIK', completed=((True,) * 26, (), ()))],
        [saved('Nowhere'), saved('KARAMJA', rewarded=(True, False))],
        [{'KARAMJA': {'startedLevels': [1], 'completedLevels': [], 'rewardedLevels': []}}],
        [{'KARAMJA': {'completedLevels': [], 'rewardedLevels': []}}],
        [{'KARAMJA': {'startedLevels': [], 'completedLevels': [[True]], 'rewardedLevels': [False]}, 'VARROCK': {}}],
        [{}],
        [],
    ]
    for data in parses:
        case('diary', [a('diary.parse', data=data), a('completedLevels'), a('diary.parse', 1, data=data), a('completedLevels')], nodes)
    # The tab and the task bookkeeping.
    case('diary', [a('diary.openTab')], nodes)
    case('diary', started('VARROCK', [1]) + started('FALADOR') + complete('FALADOR', 0) + complete('FALADOR', 1) + complete('FALADOR', 2) +
         started('LUMBRIDGE', [0]) + complete('LUMBRIDGE', 0) + [a('diary.openTab')], nodes, npcNames={'375': 'Redbeard Frank'})
    for type in ['KARAMJA', 'LUMBRIDGE', 'SEERS_VILLAGE']:
        for level in [0, 1, 2]:
            last = TASK_COUNTS[type][level] - 1
            steps = [a('diary.updateTask', type=type, level=level, index=0, complete=False), a('diary.updateTask', type=type, level=level, index=0,
                                                                                               complete=True)]
            steps += complete(type, level, last) + [a('diary.updateTask', type=type, level=level, index=last, complete=True),
                                                    a('diary.updateTask', type=type, level=level, index=last, complete=False)]
            case('diary', steps, nodes, npcNames={'1055': 'Pirate Jackie the Fruit', '7969': 'Explorer Jack'})
    for type in ['VARROCK', 'FREMENNIK']:
        for level in [0, 2]:
            last = TASK_COUNTS[type][level] - 1
            steps = complete(type, level, last) + [a('diary.finishTask', type=type, level=level, index=last), a('completedLevels'),
                                                   a('diary.finishTask', type=type, level=level, index=last), a('completedLevels'),
                                                   a('ad.resetTask', type=type, level=level, index=0), a('diary.finishTask', type=type, level=level,
                                                                                                         index=0), a('completedLevels'),
                                                   a('diary.finishTask', type=type, level=level, index=24), a('completedLevels'),
                                                   a('diary.finishTask', 1, type=type, level=level, index=1), a('diary.finishTask', type=type,
                                                                                                              level=level, index=25)]
            case('diary', steps, nodes)
    case('diary', [a('diary.finishTask', type='KARAMJA', level=0, index=3), a('ad.resetTask', type='KARAMJA', level=0, index=3),
                   a('diary.finishTask', type='KARAMJA', level=0, index=4), a('completedLevels')] + complete('KARAMJA', 0) +
         [a('diary.finishTask', type='KARAMJA', level=0, index=3), a('completedLevels'), a('ad.resetTask', type='KARAMJA', level=0, index=5),
          a('diary.finishTask', type='KARAMJA', level=0, index=5), a('completedLevels')], nodes)
    # The shared list follows all 25 slots of a level, not its task count.
    for type, level in [('KARAMJA', 0), ('SEERS_VILLAGE', 2)]:
        slots = [a('ad.setCompleted', type=type, level=level, index=i) for i in range(23)]
        case('diary', slots + [a('diary.finishTask', type=type, level=level, index=23), a('completedLevels'),
                               a('diary.finishTask', type=type, level=level, index=24), a('completedLevels'),
                               a('diary.finishTask', 1, type=type, level=level, index=24), a('ad.resetTask', type=type, level=level, index=3),
                               a('diary.finishTask', type=type, level=level, index=3), a('completedLevels'),
                               a('diary.finishTask', type=type, level=1, index=0), a('completedLevels')], nodes)
        case('diary', slots + [a('diary.finishTask', type=type, level=level, index=24), a('completedLevels'),
                               a('diary.finishTask', type=type, level=level, index=23), a('completedLevels'),
                               a('ad.resetTask', type=type, level=level, index=0), a('diary.finishTask', type=type, level=level, index=0),
                               a('completedLevels')], nodes)
    case('diary', [a('diary.updateTask', type=None, level=0, index=0),a('diary.finishTask', type=None, level=0, index=0),
                   a('diary.hasCompletedTask', type=None), a('diary.setStarted', type=None), a('diary.setCompleted', type=None)], nodes)
    case('diary', [a('diary.updateTask', type='KARAMJA', level=l, index=i, complete=True) for l, i in [(3, 0), (-1, 0), (0, 25), (0, -1)]] +
         [a('diary.hasCompletedTask', type='KARAMJA', level=l, index=i) for l, i in [(0, 0), (2, 24), (3, 0), (0, 25)]] +
         [a('diary.setStarted', type='KARAMJA', level=l) for l in [0, 2, 3]] + [a('diary.setCompleted', type='VARROCK', level=2, index=24),
                                                                               a('diary.setCompleted', type='VARROCK', level=2, index=25)], nodes)
    # Reward checks: gloves, armour, mining and smithing doubles.
    gloves = [None, 11136, 11138, 11140, 11137, 1059]
    plates = [None, 11756, 11757, 11758, 11753, 1127]
    for glove in gloves:
        for plate in plates:
            equipment = ([[9, glove, 1]] if glove else []) + ([[4, plate, 1]] if plate else [])
            case('diary', [a('diary.hasGlove'), a('diary.getKaramjaGlove'), a('diary.hasArmour'), a('diary.getArmour')],
                 [P(0, equipment=equipment)])
    for plate in plates:
        equipment = [[4, plate, 1]] if plate else []
        case('diary', [a('diary.checkMiningReward', reward=r) for r in [436, 438, 440, 442, 443, 444, 447, 448, 449, 450, 451, 453]] +
             [a('diary.checkSmithReward', bar=b) for b in BARS + [None]], [P(0, equipment=equipment)])
    case('diary', [a('diary.getArmour'), a('diary.getKaramjaGlove')], [P(0, equipment=[[4, 11758, 1], [0, 11756, 1], [9, 11140, 1], [1, 11136, 1]])])
    case('diary', [a('diary.getArmour')], [P(0, equipment=[[4, 11757, 1]])], hooks=[hook('get', [a('invSet', container='equipment', slot=4, item=None)])])
    # Completion of whole diaries.
    for type in TYPES:
        steps = []
        for level in range(3):
            steps += complete(type, level)
            steps += [a('diary.isComplete', type=type), a('ad.isComplete', type=type), a('ad.getLevel', type=type)]
        case('diary', steps, nodes)
    # AchievementDiary members.
    for type in ['KARAMJA', 'FREMENNIK', 'LUMBRIDGE']:
        case('diary', [a('ad.open', type=type)], nodes)
        case('diary', started(type, [0]) + complete(type, 0, 3) + [a('ad.open', type=type)], nodes)
        case('diary', started(type) + complete(type, 0) + complete(type, 1) + complete(type, 2) + [a('ad.open', type=type)], nodes)
    case('diary', [a('assist.module', 0, target=1), a('ad.open', type='VARROCK')], [P(0), P(1), N(2)])
    case('diary', [a('ad.open', type='SEERS_VILLAGE')], nodes, hooks=[hook('isOpened', [a('warn.open')])])
    case('diary', [a('ad.open', type='VARROCK', actor=1)], nodes, failures={'string': 200})
    for type in ['FALADOR', 'SEERS_VILLAGE']:
        steps = [a('ad.isStarted', type=type), a('ad.getLevel', type=type), a('ad.getReward', type=type), a('ad.drawStatus', type=type)]
        for level in [0, 1, 2, 3, -1]:
            steps += [a('ad.isStartedLevel', type=type, level=level), a('ad.isCompleteLevel', type=type, level=level),
                      a('ad.isCompleteCumulative', type=type, level=level, cumulative=True), a('ad.isCompleteCumulative', type=type, level=level),
                      a('ad.getLevelName', type=type, level=level), a('ad.getStatus', type=type, level=level),
                      a('ad.isLevelRewarded', type=type, level=level), a('ad.isCompleteTask', type=type, level=level, index=0)]
        steps += started(type, [1]) + complete(type, 1) + [a('ad.drawStatus', type=type), a('ad.getLevel', type=type),
                                                           a('ad.isCompleteCumulative', type=type, level=1, cumulative=True),
                                                           a('ad.getStatus', type=type, level=1)]
        steps += complete(type, 0) + [a('ad.isCompleteCumulative', type=type, level=1, cumulative=True), a('ad.isCompleteCumulative', type=type,
                                                                                                           level=2, cumulative=True)]
        steps += [a('ad.setLevelRewarded', type=type, level=l) for l in [1, 0, 3]] + [a('ad.getReward', type=type), a('ad.getType', type=type)]
        steps += [a('ad.resetTask', type=type, level=1, index=0), a('ad.isLevelRewarded', type=type, level=1), a('ad.isStartedLevel', type=type, level=1),
                  a('ad.resetTask', type=type, level=0, index=30)]
        case('diary', steps, nodes)
    case('diary', [a('ad.updateTask', type='FALADOR', level=1, index=2, complete=True), a('ad.finishTask', type='FALADOR', level=1, index=3),
                   a('ad.drawStatus', type='FALADOR', actor=1), a('completedLevels')], nodes)
    # Rewards: claiming, replacing, removal, the static helpers.
    for type in ['KARAMJA', 'SEERS_VILLAGE']:
        for level in [0, 1, 2]:
            reward, lamp = REWARDS[type][level]
            previous = REWARDS[type][level - 1][0] if level else None
            holdings = [dict(), dict(inventory=[[s, 1038, 1] for s in range(27)]), dict(inventory=[[s, 1038, 1] for s in range(26)]),
                        dict(inventory=[[0, previous, 1]] if previous else [], bank=[[3, previous, 1]] if previous else []),
                        dict(equipment=[[9, previous, 1]] if previous else [], inventory=[[0, reward, 1]])]
            for held in holdings:
                steps = [a('ad.canClaimLevelRewards', type=type, level=level), a('ad.flagRewarded', type=type, level=level),
                         a('ad.canReplaceReward', type=type, level=level), a('ad.grantReplacement', type=type, level=level)]
                case('diary', steps, [P(0, **held)])
                case('diary', complete(type, 0) + complete(type, 1) + complete(type, 2) + steps +
                     [a('ad.hasClaimedLevelRewards', type=type, level=level), a('ad.hasCompletedLevel', type=type, level=level),
                      a('ad.removeRewardsFor', type=type, level=level), a('ad.canReplaceReward', type=type, level=level),
                      a('ad.grantReplacement', type=type, level=level), a('ad.grantReplacement', type=type, level=level)], [P(0, **held)])
    case('diary', complete('VARROCK', 0) + complete('VARROCK', 1) + [a('ad.hasCompletedLevel', type='VARROCK', level=l) for l in [0, 1, 2, 3, 9]] +
         [a('ad.canClaimLevelRewards', type='VARROCK', level=l) for l in [0, 1, 2]] + [a('ad.setLevelRewarded', type='VARROCK', level=1)] +
         [a('ad.canClaimLevelRewards', type='VARROCK', level=l) for l in [0, 1]] + [a('ad.canReplaceReward', type='VARROCK', level=0),
                                                                                    a('ad.getRewards', type='VARROCK', level=1),
                                                                                    a('ad.canClaimLevelRewards', type='VARROCK', level=3)],
         nodes)
    case('diary', [a('ad.addRewardsFor', type='FALADOR', level=0), a('ad.addRewardsFor', type='FALADOR', level=1)],
         [P(0, inventory=[[s, 1038, 1] for s in range(25)])], answers={})
    case('diary', [a('ad.addRewardsFor', type='FALADOR', level=0)], [P(0)], failures={'add': 2})
    case('diary', [a('ad.addRewardsFor', type='FALADOR', level=0)], [P(0, inventory=[[s, 1038, 1] for s in range(26)])],
         hooks=[hook('add', [a('invSet', container='inventory', slot=27, item=[1, 1])])])
    case('diary', [a('ad.removeRewardsFor', type='FALADOR', level=0)], [P(0, bank=[[0, 14577, 1]], equipment=[[1, 14577, 1]])])
    case('diary', [a('ad.removeRewardsFor', type='FALADOR', level=0)], [P(0, equipment=[[1, 14577, 1]])])
    case('diary', [a('ad.removeRewardsFor', type='FALADOR', level=0)], [P(0, equipment=[[1, 14577, 1]])],
         hooks=[hook('remove', [a('setAttribute', key='debug', value=True)])])
    case('diary', [a('setAttribute', key='debug', value=True), a('ad.removeRewardsFor', type='FALADOR', level=0), a('setAttribute', key='debug', value=7),
                   a('ad.removeRewardsFor', type='FALADOR', level=1)], [P(0, inventory=[[0, 14577, 1], [1, 14578, 1]])])
    everything = [[s, REWARDS[t][l][i], 1] for s, (t, l, i) in enumerate((t, l, i) for t in TYPES for l in range(3) for i in range(2)) if s < 28]
    case('diary', [a('diary.resetRewards')], [P(0, inventory=everything[:28], bank=everything, equipment=[[9, 11136, 1], [4, 11756, 1]])])
    case('diary', [a('diary.resetRewards')], [P(0, inventory=[[0, 14631, 2]], bank=[[0, 14631, 1]])])
    for event, at in [('contains', 3), ('remove', 1), ('npcName', 1), ('sendMessages', 1), ('string', 3), ('openTab', 1), ('isArtificial', 1),
                      ('freeSlots', 1), ('add', 1), ('get', 1), ('isOpened', 1), ('runScript', 1)]:
        case('diary', started('KARAMJA') + complete('KARAMJA', 0, 9) + [a('diary.openTab'), a('diary.finishTask', type='KARAMJA', level=0, index=9),
                                                                         a('ad.flagRewarded', type='KARAMJA', level=0), a('diary.getArmour'),
                                                                         a('ad.open', type='KARAMJA'), a('diary.resetRewards')],
             [P(0, equipment=[[4, 11756, 1]], inventory=[[0, 11136, 1]])], failures={event: at})
