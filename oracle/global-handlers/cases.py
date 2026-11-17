"""Designed cases for the global-handlers oracle (see generate.py).

A case sets up the recording world (nodes, containers, attributes, answers, values), registers one or more original
classes (`define`), then runs operations against what they registered: an interface button/open/close listener, a
component plugin, an option handler, an InteractionListener callback, a stored task, pulse body or close event, the login
plugin, or a static helper. `sweep` marks the operation whose recorded calls fail one at a time in the second pass.
"""
import pathlib

H = 'content.global.handlers.'
CLASSES = {
    'attack': H + 'player.AttackOptionPlugin',
    'follow': H + 'player.FollowOptionPlugin',
    'request': H + 'player.RequestOption',
    'logout': H + 'iface.tabs.LogoutTabInterface',
    'settings': H + 'iface.tabs.SettingsTabInterface',
    'stats': H + 'iface.tabs.StatsTabInterface',
    'equipment': H + 'iface.tabs.EquipmentTabInterface',
    'combat': H + 'iface.tabs.CombatTabInterface',
    'magic': H + 'iface.tabs.MagicTabInterface',
    'prayer': H + 'iface.tabs.PrayerTabInterface',
    'quest': H + 'iface.tabs.QuestTabInterface',
    'clan': H + 'iface.tabs.ClanTabInterface',
    'emote': H + 'iface.tabs.EmoteTabInterface',
    'music': H + 'iface.tabs.MusicTabInterface',
    'main': H + 'iface.MainGameInterface',
    'login': H + 'iface.LoginInterface',
    'death': H + 'iface.DeathInterface',
    'design': H + 'iface.CharacterDesignInterface',
    'worldmap': H + 'iface.WorldMapInterface',
    'experience': H + 'iface.ExperienceInterface',
    'rules': H + 'iface.RulesListener',
    'itemselect': H + 'iface.GenericItemSelect',
    'autocast': H + 'iface.AutocastSelectPlugin',
    'teleother': H + 'iface.TeleotherInterface',
    'assist': H + 'iface.RequestAssistInterface',
    'depositbox': H + 'iface.bank.BankDepositBoxInterface',
    'charge': H + 'iface.bank.BankChargeInterface',
    'booth': H + 'scenery.BankBoothListener',
    'chest': H + 'scenery.BankChestListener',
    'boxes': H + 'scenery.BankDepositBoxListener',
    'lookat': H + 'scenery.LookAtOptionPlugin',
    'search': H + 'scenery.SearchOptionPlugin',
    'ladder': H + 'scenery.LadderManagingPlugin',
    'npcdeposit': H + 'npc.NPCDepositListener',
    'dummy': H + 'npc.DummyAttackPlugin',
}
BY_CLASS = {v: k for k, v in CLASSES.items()}

NEEDED_EVENTS = ['ifaceOn', 'ifaceOnOpen', 'ifaceOnClose', 'componentPlugin', 'setHandler', 'sceneryOptionHandler', 'sceneryHandler',
                 'on', 'defineInteraction', 'onUseAnyWith', 'attack', 'movementPulse', 'face', 'request', 'botInfo', 'activityStart',
                 'setLootShareDelay', 'setLootSharePoints', 'doubleItemDialogue', 'itemDialogue', 'disconnect', 'toggleAcceptAid',
                 'flashingIcons', 'runScript', 'ifaceSettings', 'log', 'unequip', 'operateHandle', 'generateItems', 'submit',
                 'toggleRetaliating', 'swing', 'castSpell', 'prayerToggle', 'drawJournal', 'questRequirements', 'diaryOpen',
                 'msClanRename', 'clanDelete', 'msClanSetting', 'emote', 'unlock', 'play', 'setCloseEvent', 'interfaceConfig',
                 'scriptConfigs', 'configureGameWorld', 'setDisableDeathScreen', 'characterDesign', 'updateSceneGraph',
                 'selectionCallback', 'callback', 'playAudio', 'componentOpen', 'random', 'lockAll', 'packetString',
                 'closeSingleTab', 'openTabWeapon', 'visualize', 'assistToggle', 'bankAddItem', 'transferX', 'dumpBob',
                 'npcDialogue', 'openBank', 'openCollectionBox', 'openDialogue', 'faceLocation',
                 'removeTabs', 'openDefaultTabs', 'add', 'getObject', 'teleportPermitted', 'clippingFlag', 'setTeleportLocation', 'finishTask', 'zogreLadder', 'sendOptions', 'setDialogue', 'dialogueEnd', 'addExperience', 'openDepositBox']


def define(*names):
    return [dict(op='define', **{'class': CLASSES[n]}) for n in names]


def btn(iface, button, opcode=155, slot=0, item=-1):
    return dict(op='button', iface=iface, button=button, opcode=opcode, slot=slot, item=item)


def plugin(iface, button, opcode=155, slot=0, item=-1):
    return dict(op='plugin', iface=iface, button=button, opcode=opcode, slot=slot, item=item)


def task(n, **extra):
    return dict(op='task', n=n, **extra)


def case(group, name, ops, sweep=None, **state):
    out = dict(group=group, name=name, ops=ops)
    out.update(state)
    return out, sweep


def last(ops):
    return len(ops) - 1


def scan_order():
    """The original's scan order (crates/server/src/startup/scan_order.txt, from the world-trace boot record)."""
    text = (pathlib.Path(__file__).resolve().parents[2] / 'crates/server/src/startup/scan_order.txt').read_text(encoding='utf-8')
    section, order = None, []
    for line in text.splitlines():
        if line.startswith('['):
            section = line
        elif section in ('[scan.contentInterfaces]', '[scan.initializable]') and line in BY_CLASS:
            order.append((section, line))
    return order


def registration():
    for n in CLASSES:
        yield case('registration', n, define(n))
    # Every class in the original's scan order: content interfaces first, then @Initializable plugins. RequestOption
    # appears in both lists; as an @Initializable it is constructed but is no Plugin, so it registers only once.
    ops, seen = [], set()
    for section, cls in scan_order():
        if section == '[scan.initializable]' and cls == CLASSES['request']:
            continue
        if cls in seen:
            continue
        seen.add(cls)
        ops.append(dict(op='define', **{'class': cls}))
    yield case('registration', 'scan order', ops)
    # A ComponentPlugin defined twice is two objects. (Non-capturing Kotlin lambdas are singletons across repeated
    # definitions; the port makes one callback object per registration, and the original defines each class once.)
    yield case('registration', 'defined twice', define('combat', 'combat', 'autocast', 'autocast'))
    yield case('registration', 'booth notes disabled', define('booth'), constants=dict(BANK_BOOTH_NOTE_ENABLED=False))


T = dict(kind='player', name='Other', location=[3223, 3218, 0])
N = dict(kind='npc', id=1, name='Man', location=[3220, 3218, 0])


def player_options():
    nodes = dict(t=T, n=N, b=dict(kind='bot', name='Bot'))
    yield case('attack', 'player', define('attack') + [dict(op='handler', via='option:Attack', node='t', option='attack')], 1, nodes=nodes)
    yield case('attack', 'npc', define('attack') + [dict(op='handler', via='option:Attack', node='n', option='attack')], 1, nodes=nodes)
    yield case('attack', 'props', define('attack') + [dict(op='props', via='option:Attack', node='t'), dict(op='destination', via='option:Attack', node='t')], nodes=nodes)
    follow = define('follow') + [dict(op='handler', via='option:Follow', node='t', option='follow'), task(0), dict(op='stop', n=0), task(0)]
    yield case('follow', 'player', follow, 1, nodes=nodes)
    yield case('follow', 'pulse body', follow, 2, nodes=nodes)
    yield case('follow', 'stop', follow, 3, nodes=nodes)
    yield case('follow', 'npc', define('follow') + [dict(op='handler', via='option:Follow', node='n', option='follow')], nodes=nodes)
    yield case('follow', 'null', define('follow') + [dict(op='handler', via='option:Follow', option='follow'), task(0), dict(op='stop', n=0)], nodes=nodes)
    yield case('follow', 'props', define('follow') + [dict(op='props', via='option:Follow', node='t'), dict(op='destination', via='option:Follow', node='t')], nodes=nodes)
    for n, target in [(0, 't'), (1, 't'), (1, 'b'), (0, 'b'), (0, 'n'), (1, 'n'), (0, None), (1, None)]:
        op = dict(op='interact', n=n)
        if target:
            op['node'] = target
        yield case('request', f'{n} {target}', define('request') + [op], 1, nodes=nodes)
    yield case('request', 'answer false', define('request') + [dict(op='interact', n=0, node='t')], nodes=nodes, answers=dict(request=False))


def login_plugin():
    yield case('login-plugin', 'artificial', [dict(op='login')], 0, answers=dict(isArtificial=True))
    yield case('login-plugin', 'invalid', [dict(op='login')], 0, answers=dict(validLogin=False))
    yield case('login-plugin', 'plain', [dict(op='login')], 0)
    yield case('login-plugin', 'dev fight caves falconry dragon', [dict(op='login')], 0,
               answers=dict(devMode=True, dragonSlayerAttribute=True), attributes=dict(p={'fc_wave': 3, 'falconry': True}))
    yield case('login-plugin', 'fc wave zero', [dict(op='login')], attributes=dict(p={'fc_wave': 0, 'falconry': False}))
    yield case('login-plugin', 'fc wave minus', [dict(op='login')], attributes=dict(p={'fc_wave': -1}))
    yield case('login-plugin', 'fc wave text', [dict(op='login')], attributes=dict(p={'fc_wave': 'x'}))
    yield case('login-plugin', 'falconry text', [dict(op='login')], attributes=dict(p={'falconry': 'x'}))
    for points in [0, 1, 9, 10, 105, 2147483647, -7]:
        yield case('login-plugin', f'loot share decay {points}', [dict(op='login')], values=dict(lootShareDelay=1, lootSharePoints=points))
    yield case('login-plugin', 'loot share decay sweep', [dict(op='login')], 0, values=dict(lootShareDelay=1, lootSharePoints=105))
    yield case('login-plugin', 'loot share future', [dict(op='login')], values=dict(lootShareDelay=9000000000000000, lootSharePoints=50))
    yield case('login-plugin', 'loot share negative', [dict(op='login')], values=dict(lootShareDelay=-5, lootSharePoints=50))
    qp = dict(hasCompletedAll=False, anyInEquipment=True)
    yield case('login-plugin', 'qp completed', [dict(op='login')], answers=dict(hasCompletedAll=True, anyInEquipment=True))
    yield case('login-plugin', 'qp none worn', [dict(op='login')], answers=dict(hasCompletedAll=False, anyInEquipment=False))
    yield case('login-plugin', 'qp both inventory', [dict(op='login')], 0, answers=dict(qp))
    yield case('login-plugin', 'qp bank', [dict(op='login')], 0, answers=dict(qp, **{'addItem:INVENTORY': [False, True]}),
               values={'itemName:9813': 'Quest point cape', 'itemName:9814': 'Quest point hood'})
    yield case('login-plugin', 'qp wise old man', [dict(op='login')], 0, answers=dict(qp, **{'addItem:INVENTORY': False, 'addItem:BANK': False}))
    yield case('login-plugin', 'qp hood only', [dict(op='login')], 0, answers=dict(qp, removeItem=[False, True]))
    yield case('login-plugin', 'qp cape only', [dict(op='login')], answers=dict(qp, removeItem=[True, False], **{'addItem:INVENTORY': False}))
    yield case('login-plugin', 'qp none removed', [dict(op='login')], 0, answers=dict(qp, removeItem=False))


def tabs():
    # LogoutTabInterface
    for name, answers in [('cannot', dict(canLogout=False)), ('combat', {'inCombat:p': True}), ('teleporting', dict(isTeleporting=True)), ('logout', {})]:
        yield case('logout', name, define('logout') + [btn(182, 6)], 1, answers=answers)
    yield case('logout', 'other button', define('logout') + [btn(182, 5)])
    # SettingsTabInterface
    for b in [3, 4, 5, 6, 7, 8, 16, 18, 2, 17]:
        yield case('settings', f'button {b}', define('settings') + [btn(261, b)], 1)
    yield case('settings', 'aid restricted', define('settings') + [btn(261, 7)], 1, answers=dict(checkRestriction=True))
    # StatsTabInterface
    for b in [125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 124, 149, 0]:
        yield case('stats', f'guide {b}', define('stats') + [btn(320, b)], 1 if b == 125 else None)
    for b, skill in [(125, 0), (133, 3), (148, 23), (132, 22)]:
        yield case('stats', f'level up {b}', define('stats') + [btn(320, b)], 1, attributes=dict(p={f'levelup:{skill}': True}))
    yield case('stats', 'level up false', define('stats') + [btn(320, 126)], attributes=dict(p={'levelup:2': False}))
    yield case('stats', 'level up wrong type', define('stats') + [btn(320, 126)], attributes=dict(p={'levelup:2': 1}))
    yield case('stats', 'menu', define('stats') + [btn(320, 130), btn(499, 12), btn(499, 10)], 2)
    yield case('stats', 'menu unset', define('stats') + [btn(499, 9)], 1)
    yield case('stats', 'menu text', define('stats') + [btn(499, 9)], attributes=dict(p={'skillMenu': 'x'}))
    yield case('stats', 'level up close', define('stats') + [btn(740, 3), btn(740, 4)], 1)
    yield from equipment()
    yield from combat()
    # MagicTabInterface
    for iface in [192, 193, 430]:
        yield case('magic', f'cast {iface}', define('magic') + [btn(iface, 21)], 1, answers=dict(castSpell=True))
    yield case('magic', 'cast false', define('magic') + [btn(192, 3)], values=dict(ticks=10), attributes=dict(p={'magic:delay': 10}))
    yield case('magic', 'delayed', define('magic') + [btn(192, 3)], 1, values=dict(ticks=10), attributes=dict(p={'magic:delay': 11}))
    yield case('magic', 'delay text', define('magic') + [btn(192, 3)], attributes=dict(p={'magic:delay': 'x'}))
    # PrayerTabInterface
    yield case('prayer', 'unknown', define('prayer') + [btn(271, 1)], 1)
    yield case('prayer', 'toggle', define('prayer') + [btn(271, 5)], 1, answers=dict(prayerType='PROTECT_ITEMS', prayerToggle=True))
    yield case('prayer', 'refused', define('prayer') + [btn(271, 5)], answers=dict(prayerType='THICK_SKIN', prayerToggle=False))
    # QuestTabInterface
    yield case('quest', 'diary tab', define('quest') + [btn(274, 3)], 1)
    yield case('quest', 'journal', define('quest') + [btn(274, 10)], 1, answers=dict(questForButton="Cook's Assistant", questStage=20))
    yield case('quest', 'requirements', define('quest') + [btn(274, 12)], 1)
    yield case('quest', 'quest tab', define('quest') + [btn(259, 8)], 1)
    yield case('quest', 'diary', define('quest') + [btn(259, 11)], 1, answers=dict(diaryForChild='KARAMJA'))
    yield case('quest', 'diary unknown', define('quest') + [btn(259, 1)], 1)
    yield case('quest', 'diary missing', define('quest') + [btn(259, 11)], answers=dict(diaryForChild='VARROCK', getDiary=False))
    yield from clan()
    # EmoteTabInterface
    for b in [1, 2, 37, 0]:
        yield case('emote', f'button {b}', define('emote') + [btn(464, b)], 1 if b == 1 else None)
    yield from music()


def equipment():
    item = lambda i, **extra: dict(kind='item', id=i, **extra)
    nodes = dict(w=item(1351), r=item(1917), s=item(4151))
    kept = lambda *keys: dict(kept=list(keys) + [None] * (4 - len(keys)))
    yield case('equipment', 'death plain', define('equipment') + [dict(op='open', iface=102)], 1, nodes=nodes, containers=kept('w'))
    yield case('equipment', 'death protect', define('equipment') + [dict(op='open', iface=102)], 1, nodes=nodes,
               containers=kept('w', 's'), answers=dict(prayerActive=True))
    yield case('equipment', 'death skulled', define('equipment') + [dict(op='open', iface=102)], 1, nodes=nodes,
               containers=kept('w', 's', 'r'), answers=dict(isSkulled=True, zoneType=1))
    yield case('equipment', 'death three kept', define('equipment') + [dict(op='open', iface=102)], nodes=nodes, containers=kept('w', 's', 'r'))
    yield case('equipment', 'death five', define('equipment') + [dict(op='open', iface=102)], nodes=nodes,
               containers=dict(kept=['w', 's', 'r', 'w', 's'], lost=['r', None]), answers=dict(isSkulled=True))
    yield case('equipment', 'death five safe', define('equipment') + [dict(op='open', iface=102)], nodes=nodes,
               containers=dict(kept=['w', 's', 'r', 'w', 's']), answers=dict(isSkulled=True, zoneType=2))
    yield case('equipment', 'death bob', define('equipment') + [dict(op='open', iface=102)], 1, nodes=nodes,
               containers=dict(kept=[None] * 4, bob=['w', None]), answers=dict(hasFamiliar=True, isBurdenBeast=True))
    yield case('equipment', 'death empty bob', define('equipment') + [dict(op='open', iface=102)], nodes=nodes,
               answers=dict(hasFamiliar=True, isBurdenBeast=True))
    yield case('equipment', 'death familiar', define('equipment') + [dict(op='open', iface=102)], nodes=nodes,
               answers=dict(hasFamiliar=True, isBurdenBeast=False))
    yield case('equipment', 'death not a beast', define('equipment') + [dict(op='open', iface=102)], nodes=nodes,
               answers=dict(hasFamiliar=True, isBurdenBeast=True, beastObject=False))
    worn = dict(equipment=[None, None, None, 's'] + [None] * 10)
    yield case('equipment', 'worn unequip', define('equipment') + [btn(387, 28, 81, 3, 4151)], 1, nodes=nodes, containers=worn)
    yield case('equipment', 'worn operate listener', define('equipment') + [btn(387, 28, 206, 3, 4151)], 1, nodes=nodes, containers=worn,
               answers={'listeners:operate': True})
    yield case('equipment', 'worn operate handler', define('equipment') + [btn(387, 28, 206, 3, 4151)], 1, nodes=nodes, containers=worn,
               answers=dict(operateHandler=True, operateHandle=True))
    yield case('equipment', 'worn operate handler false', define('equipment') + [btn(387, 28, 206, 3, 4151)], nodes=nodes, containers=worn,
               answers=dict(operateHandler=True, operateHandle=False))
    yield case('equipment', 'worn operate nothing', define('equipment') + [btn(387, 28, 206, 3, 4151)], 1, nodes=nodes, containers=worn)
    yield case('equipment', 'worn operate empty', define('equipment') + [btn(387, 28, 206, 2, 4151)], nodes=nodes, containers=worn)
    yield case('equipment', 'worn operate outside', define('equipment') + [btn(387, 28, 206, 40, 4151), btn(387, 28, 206, -1, 4151)], nodes=nodes, containers=worn)
    yield case('equipment', 'worn other opcode', define('equipment') + [btn(387, 28, 155, 3, 4151)], nodes=nodes, containers=worn)
    for b in [52, 55, 27]:
        yield case('equipment', f'worn button {b}', define('equipment') + [btn(387, b)], 1 if b != 27 else None)
    yield case('equipment', 'screen open', define('equipment') + [dict(op='open', iface=667)], 1)
    yield case('equipment', 'screen examine', define('equipment') + [btn(667, 14, 9, 3)], 1, nodes=nodes, containers=worn,
               answers={'examine:s': 'A whip.'})
    yield case('equipment', 'screen examine empty', define('equipment') + [btn(667, 14, 9, 2)], nodes=nodes, containers=worn)
    yield case('equipment', 'screen examine null', define('equipment') + [btn(667, 14, 9, 3)], nodes=nodes, containers=worn,
               answers={'examine:s': None})
    yield case('equipment', 'screen unequip', define('equipment') + [btn(667, 14, 155, 3, 4151)], 1, nodes=nodes, containers=worn)
    yield case('equipment', 'screen operate', define('equipment') + [btn(667, 14, 196, 3, 4151)], nodes=nodes, containers=worn)
    yield case('equipment', 'screen other', define('equipment') + [btn(667, 13, 155, 3, 4151), btn(667, 14, 124, 3)], nodes=nodes, containers=worn)
    yield case('equipment', 'screen close', define('equipment') + [dict(op='close', iface=667)], 1)
    inv = dict(inventory=['w', 'r', 's', None] + [None] * 24)
    yield case('equipment', 'inventory open', define('equipment') + [dict(op='open', iface=670)], 1, nodes=nodes, containers=inv)
    yield case('equipment', 'inventory examine', define('equipment') + [btn(670, 0, 9, 1)], 1, nodes=nodes, containers=inv)
    yield case('equipment', 'inventory equip wield', define('equipment') + [btn(670, 0, 155, 0)], 1, nodes=nodes, containers=inv,
               answers={'options:w': ['Wield', 'Use', None, 'Drop']})
    yield case('equipment', 'inventory equip beer', define('equipment') + [btn(670, 0, 155, 1)], 1, nodes=nodes, containers=inv,
               answers={'options:r': ['Drink', None]})
    yield case('equipment', 'inventory equip refused', define('equipment') + [btn(670, 0, 155, 2)], 1, nodes=nodes, containers=inv,
               answers={'options:s': ['Use', 'equip', None]})
    yield case('equipment', 'inventory equip null options', define('equipment') + [btn(670, 0, 155, 2)], nodes=nodes, containers=inv)
    yield case('equipment', 'inventory equip empty', define('equipment') + [btn(670, 0, 155, 3)], nodes=nodes, containers=inv)
    yield case('equipment', 'inventory other', define('equipment') + [btn(670, 0, 196, 1)], nodes=nodes, containers=inv)
    yield case('equipment', 'inventory examine empty', define('equipment') + [btn(670, 0, 9, 3)], nodes=nodes, containers=inv)


def combat():
    nodes = dict(v=dict(kind='npc', id=2, name='Goblin'))
    for b in [7, 9, 24, 26, 27]:
        yield case('combat', f'retaliate {b}', define('combat') + [plugin(90, b), task(0)], 1 if b == 7 else None)
    yield case('combat', 'retaliate body', define('combat') + [plugin(90, 7), task(0)], 2)
    special = define('combat') + [plugin(92, 10), task(0)]
    yield case('combat', 'special no weapon', special, 2, answers=dict(weaponInterface=None))
    yield case('combat', 'special no bar', special, 2)
    yield case('combat', 'special untoggled', special, 2, answers=dict(isSpecialBar=True))
    yield case('combat', 'special no handler', special, 2, answers=dict(isSpecialBar=True, isSpecialToggled=True))
    yield case('combat', 'special not instant', special, 2, answers=dict(isSpecialBar=True, isSpecialToggled=True, getSpecial='dds'))
    yield case('combat', 'special instant', special, 2, nodes=nodes, containers=dict(equipment=[None, None, None, 'x'] + [None] * 10),
               answers=dict(isSpecialBar=True, isSpecialToggled=True, getSpecial='dds', fireEvent=True, victim='v'))
    yield case('combat', 'special instant no victim', special, answers=dict(isSpecialBar=True, isSpecialToggled=True, getSpecial='dds', fireEvent=True))
    yield case('combat', 'special event text', special, answers=dict(isSpecialBar=True, isSpecialToggled=True, getSpecial='dds', fireEvent='true'))
    for b in [11, 8, 85]:
        yield case('combat', f'special {b}', define('combat') + [plugin(81, b)])
    yield case('combat', 'style no weapon', define('combat') + [plugin(92, 2)], 1, answers=dict(weaponInterface=None))
    yield case('combat', 'style refused', define('combat') + [plugin(92, 2)], 1)
    yield case('combat', 'style autocast select', define('combat') + [plugin(90, 4), plugin(90, 5)], 1, answers=dict(setAttackStyle=True))
    yield case('combat', 'style autocast cleared', define('combat') + [plugin(90, 3)], 1, answers=dict(setAttackStyle=True, autocastSpell=True))
    yield case('combat', 'style plain', define('combat') + [plugin(90, 0), plugin(90, 1)], 1, answers=dict(setAttackStyle=True))
    yield case('combat', 'unregistered', define('combat') + [plugin(95, 1)])


def clan():
    yield case('clan', 'setup', define('clan') + [btn(589, 9)], 1)
    yield case('clan', 'setup blocked', define('clan') + [btn(589, 9)], 1, answers=dict(opened=True))
    yield case('clan', 'lootshare', define('clan') + [btn(589, 14)], 1)
    yield case('clan', 'join button', define('clan') + [btn(589, 3)])
    prefix = define('clan') + [btn(590, 22, 155), task(0, value='my_clan name')]
    yield case('clan', 'prefix', prefix, 1)
    yield case('clan', 'prefix entered', prefix, 2, answers=dict(wcEnabled=True))
    yield case('clan', 'prefix enabled', prefix, 2, values=dict(clanName='Old name'))
    yield case('clan', 'prefix number', define('clan') + [btn(590, 22, 155), task(0, value=12)])
    yield case('clan', 'prefix unicode', define('clan') + [btn(590, 22, 155), task(0, value='ßtraße_x')])
    yield case('clan', 'disable', define('clan') + [btn(590, 22, 196)], 1, answers=dict(wcEnabled=True), values=dict(commClanName='Mine'))
    yield case('clan', 'disable no ms', define('clan') + [btn(590, 22, 196)])
    yield case('clan', 'prefix other opcode', define('clan') + [btn(590, 22, 124)])
    for b in [23, 24, 25, 26]:
        for opcode in [155, 196, 124, 199, 234, 168, 166, 64, 53, 9]:
            yield case('clan', f'rank {b} {opcode}', define('clan') + [btn(590, b, opcode)], 1 if opcode == 196 else None)
    yield case('clan', 'coinshare', define('clan') + [btn(590, 33)], 1)
    yield case('clan', 'other', define('clan') + [btn(590, 30)])
    yield case('clan', 'no clan', define('clan') + [btn(590, 23, 155), btn(590, 30)], answers=dict(clanNull=True))
    yield case('clan', 'no clan prefix', define('clan') + [btn(590, 22, 155), task(0, value='x')], 2, answers=dict(clanNull=True, wcEnabled=True))


def music():
    songs = [[100, 4], [101, 7], [102, 4]]
    yield case('music', 'other opcode', define('music') + [btn(187, 11, 196)])
    yield case('music', 'loop', define('music') + [btn(187, 11)], 1)
    yield case('music', 'play', define('music') + [btn(187, 1, 155, 4)], 1, unlocked={'4': [100, 4]})
    yield case('music', 'locked', define('music') + [btn(187, 1, 155, 4)], 1, songs=songs)
    yield case('music', 'admin unlock', define('music') + [btn(187, 1, 155, 4)], 1, songs=songs, answers=dict(isAdmin=True))
    yield case('music', 'admin none', define('music') + [btn(187, 1, 155, 9)], songs=songs, answers=dict(isAdmin=True))
    yield case('music', 'other button', define('music') + [btn(187, 2)])


def interfaces():
    # MainGameInterface
    yield case('main', 'report', define('main') + [btn(751, 27), task(0, iface=553)], 1)
    yield case('main', 'report close', define('main') + [btn(751, 27), task(0, iface=553)], 2)
    yield case('main', 'report moderator', define('main') + [btn(751, 27)], 1, answers=dict(rights='PLAYER_MODERATOR'))
    yield case('main', 'report null', define('main') + [btn(751, 27)], answers=dict(openNull=True))
    yield case('main', 'filter other', define('main') + [btn(751, 26)])
    yield case('main', 'run', define('main') + [btn(750, 1)], 1)
    yield case('main', 'map fullscreen', define('main') + [btn(746, 110)], 1, nodes=dict(p=dict(location=[3093, 3493, 1])))
    yield case('main', 'map combat', define('main') + [btn(746, 110)], 1, answers={'inCombat:p': True})
    yield case('main', 'map interaction locked', define('main') + [btn(746, 110)], 1, answers=dict(interactionLocked=True))
    yield case('main', 'map movement locked', define('main') + [btn(746, 110)], 1, answers=dict(movementLocked=True))
    yield case('main', 'fullscreen other', define('main') + [btn(746, 111)])
    yield case('main', 'map', define('main') + [btn(548, 66)], 1, nodes=dict(p=dict(location=[16383, 16383, 3])))
    yield case('main', 'staff', define('main') + [btn(548, 38)], 1)
    yield case('main', 'staff enum', define('main') + [btn(548, 38)], 1, answers=dict(weaponInterface='STAFF'))
    yield case('main', 'quests', define('main') + [btn(548, 40)], 1)
    yield case('main', 'inventory', define('main') + [btn(548, 41)], 1)
    yield case('main', 'toplevel other', define('main') + [btn(548, 39)])
    # LoginInterface
    yield case('login', 'play', define('login') + [btn(378, 140), task(0)], 1)
    yield case('login', 'play task', define('login') + [btn(378, 140), task(0)], 2)
    for b in [145, 204, 1]:
        yield case('login', f'button {b}', define('login') + [btn(378, b)])
    yield case('login', 'close locked', define('login') + [dict(op='close', iface=378)], 1, answers=dict(isLocked=True))
    yield case('login', 'close unlocked', define('login') + [dict(op='close', iface=378)])
    # DeathInterface
    yield case('death', 'disable', define('death') + [btn(153, 1)], 1)
    yield case('death', 'other', define('death') + [btn(153, 2)])
    # CharacterDesignInterface
    yield case('design', 'button', define('design') + [plugin(771, 49)], 1)
    yield case('design', 'other opcode', define('design') + [plugin(771, 49, 196)])
    # WorldMapInterface
    yield case('worldmap', 'close fixed', define('worldmap') + [btn(755, 3)], 1)
    yield case('worldmap', 'close resizable', define('worldmap') + [btn(755, 3)], 1, answers=dict(isResizable=True))
    yield case('worldmap', 'other', define('worldmap') + [btn(755, 2)])
    yield from experience()
    # FriendsListInterface
    yield case('friends', 'open', [dict(op='friends')], 0, values=dict(worldId=2))
    yield case('friends', 'named', [dict(op='friends')], constants=dict(SERVER_NAME='Test'))
    yield case('friends', 'no settings', [dict(op='friends')], settings=False)
    yield from rules()
    # ScrollInterface
    yield case('scroll', 'page', [dict(op='scroll', component=220, lines=[['first', 1], ['zero', 0], ['last', 15], ['beyond', 16], ['mid', 7]])], 0)
    yield case('scroll', 'other component', [dict(op='scroll', component=221, lines=[['first', 1]])], 0)
    yield case('scroll', 'empty', [dict(op='scroll', component=220, lines=[])])
    yield from item_select()
    yield from autocast()
    # TeleotherInterface
    yield case('teleother', 'accept', define('teleother') + [btn(326, 5)], 1, answers=dict(teleport=True),
               attributes=dict(p={'t-o_location': {'location': [3210, 3424, 0]}}))
    yield case('teleother', 'accept own location', define('teleother') + [btn(326, 5)], 1)
    yield case('teleother', 'decline', define('teleother') + [btn(326, 6)], 1, answers=dict(closeManager=False))
    # RequestAssistInterface
    yield case('assist', 'no session', define('assist') + [plugin(301, 15)], 1)
    yield case('assist', 'other player', define('assist') + [plugin(301, 15)], 1, nodes=dict(t=T), answers=dict(assistSession=True, assistPlayer='t'))
    for b in [15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 14]:
        yield case('assist', f'button {b}', define('assist') + [plugin(301, b)], 1 if b == 15 else None, answers=dict(assistSession=True))
    yield from deposit_box()
    yield from charge()


def experience():
    for b in list(range(29, 53)) + [2, 28, 53, 0]:
        yield case('experience', f'select {b}', define('experience') + [plugin(134, b)], 1 if b in (29, 28) else None)
    confirm = lambda skill, **state: case('experience', f'confirm {skill} {sorted(state)}', define('experience') + [plugin(134, 2)], 1,
                                          attributes=dict(p=dict({'exp_interface:skill': skill}, **state.pop('attrs', {}))), **state)
    yield confirm(0)
    for skill, quest in [(15, 'DRUIDIC_RITUAL'), (20, 'RUNE_MYSTERIES'), (23, 'WOLF_WHISTLE')]:
        yield confirm(skill)
        yield confirm(skill, answers={f'questComplete:{quest}': True}, attrs={'caller': {'plugin': 'lamp'}})
    yield confirm(4, attrs={'caller': {'plugin': 'lamp'}})
    yield confirm(6, attrs={'caller': {'callback': 'book'}})
    yield confirm(7, attrs={'caller': 'text'})
    yield case('experience', 'confirm wrong type', define('experience') + [plugin(134, 2)], attributes=dict(p={'exp_interface:skill': 'x'}))
    yield case('experience', 'select then confirm', define('experience') + [plugin(134, 31), plugin(134, 2)], 2,
               attributes=dict(p={'caller': {'callback': 'book'}}))


def rules():
    yield case('rules', 'close unconfirmed', define('rules') + [dict(op='close', iface=384), task(0)], 1)
    yield case('rules', 'close task', define('rules') + [dict(op='close', iface=384), task(0)], 2, attributes=dict(p={'tutorial:complete': True}),
               answers=dict(random=4321))
    yield case('rules', 'close confirmed', define('rules') + [dict(op='close', iface=384)], 1, attributes=dict(p={'rules:confirmed': True}))
    yield case('rules', 'open', [dict(op='rules')], 0, attributes=dict(p={'tutorial:complete': True}), answers=dict(random=1000))
    yield case('rules', 'open pinned', [dict(op='rules')], attributes=dict(p={'tutorial:complete': True, 'rules:pin': 5555}), answers=dict(random=9999))
    yield case('rules', 'open tutorial', [dict(op='rules')], 0)
    yield case('rules', 'open confirmed', [dict(op='rules')], attributes=dict(p={'rules:confirmed': True}))
    yield case('rules', 'open pin text', [dict(op='rules')], attributes=dict(p={'tutorial:complete': True, 'rules:pin': 'x'}))
    yield case('rules', 'base', [dict(op='rules_base')], 0)


def item_select():
    yield case('itemselect', 'open', define('itemselect') + [dict(op='open', iface=12), task(0), dict(op='stop', n=0)], 1)
    yield case('itemselect', 'stop', define('itemselect') + [dict(op='open', iface=12), dict(op='stop', n=0)], 2)
    yield case('itemselect', 'no callback', define('itemselect') + [btn(12, 0, 155, 3)], 1)
    for opcode in [155, 196, 124, 199, 234, 9, 168]:
        yield case('itemselect', f'opcode {opcode}', define('itemselect') + [btn(12, 0, opcode, 5)], 1 if opcode in (155, 168) else None,
                   attributes=dict(p={'itemselect-callback': {'callback': 'pick'}}))
    yield case('itemselect', 'keep alive', define('itemselect') + [btn(12, 0, 196, 2)], 1,
               attributes=dict(p={'itemselect-callback': {'callback': 'pick'}, 'itemselect-keepalive': True}))
    yield case('itemselect', 'wrong callback', define('itemselect') + [btn(12, 0, 196, 2)], attributes=dict(p={'itemselect-callback': 'x'}))
    yield case('itemselect', 'close', define('itemselect') + [dict(op='close', iface=12)], 1,
               attributes=dict(p={'itemselect-callback': {'callback': 'pick'}, 'itemselect-keepalive': True}))


def autocast():
    yield case('autocast', 'not selecting', define('autocast') + [plugin(797, 3)], 1)
    yield case('autocast', 'select', define('autocast') + [plugin(319, 7)], 1, attributes=dict(p={'autocast_select': True}))
    yield case('autocast', 'select no weapon', define('autocast') + [plugin(310, 7)], 1, attributes=dict(p={'autocast_select': True}),
               answers=dict(weaponInterface=None))
    yield case('autocast', 'swap', define('autocast') + [plugin(406, 7)], 1, attributes=dict(p={'autocast_select': True, 'autocast_component': 3}),
               answers=dict(getTimer=True))
    yield case('autocast', 'swap no weapon', define('autocast') + [plugin(797, 7)], 1, attributes=dict(p={'autocast_select': True}),
               answers=dict(getTimer=True, weaponInterface=None))
    yield case('autocast', 'wrong type', define('autocast') + [plugin(797, 7)], attributes=dict(p={'autocast_select': 1}))


def deposit_box():
    item = dict(kind='item', id=995, amount=50)
    nodes = dict(i=item, j=dict(kind='item', id=995, amount=7), k=dict(kind='item', id=1, amount=3))
    inv = dict(inventory=['i', 'k', 'j'] + [None] * 25)
    yield case('depositbox', 'open', define('depositbox') + [dict(op='open', iface=11)], 1, constants=dict(SERVER_NAME='Test'))
    yield case('depositbox', 'beast of burden', define('depositbox') + [btn(11, 13)], 1)
    yield case('depositbox', 'empty slot', define('depositbox') + [btn(11, 15, 155, 5)], 1, nodes=nodes, containers=inv)
    yield case('depositbox', 'examine', define('depositbox') + [btn(11, 15, 168, 0)], 1, nodes=nodes, containers=inv, answers={'examine:i': 'Coins.'})
    for opcode in [155, 196, 124, 199, 234, 9]:
        ops = define('depositbox') + [btn(11, 15, opcode, 0), task(0)]
        if opcode == 234:
            ops.append(task(1))
        yield case('depositbox', f'deposit {opcode}', ops, 2 if opcode in (155, 199) else 1, nodes=nodes, containers=inv)
    yield case('depositbox', 'deposit sweep', define('depositbox') + [btn(11, 15, 155, 1), task(0)], 1, nodes=nodes, containers=inv)


def charge():
    for b in [27, 42, 28, 41, 36]:
        yield case('charge', f'too few {b}', define('charge') + [btn(619, b)], 1 if b == 28 else None, answers=dict(amountInInventory=19))
    for action in ['open', 'collect', 'other']:
        yield case('charge', f'charge {action}', define('charge') + [btn(619, 33)], 1, answers=dict(amountInInventory=20),
                   attributes=dict(p={'zmi:bankaction': action}))
    yield case('charge', 'charge unset', define('charge') + [btn(619, 39)], answers=dict(amountInInventory=25))
    yield case('charge', 'charge not removed', define('charge') + [btn(619, 39)], 1, answers=dict(amountInInventory=25, removeItem=False),
               attributes=dict(p={'zmi:bankaction': 'open'}))
    yield case('charge', 'charge wrong type', define('charge') + [btn(619, 39)], answers=dict(amountInInventory=25),
               attributes=dict(p={'zmi:bankaction': 4}))


def scenery():
    booth = dict(kind='scenery', id=2213, name='Bank booth', location=[3208, 3220, 2])
    banker = dict(kind='banker', id=494, name='Banker', location=[3208, 3221, 2])
    nodes = dict(o=booth, k=banker, m=dict(kind='npc', id=1, name='Man', location=[3209, 3220, 2]),
                 i=dict(kind='item', id=1511, amount=5), x=dict(kind='item', id=1512, amount=30))
    north = {'findNpc:3208,3221,2': 'k'}
    yield case('booth', 'quick ironman', define('booth') + [dict(op='interact', n=0, node='o', state=0)], 1, nodes=nodes, answers=dict(checkRestriction=True))
    yield case('booth', 'quick', define('booth') + [dict(op='interact', n=0, node='o', state=0)], 1, nodes=nodes)
    yield case('booth', 'quick lunar', define('booth') + [dict(op='interact', n=0, node='o', state=0)], 1, nodes=nodes,
               answers=dict(north, lunarRestriction=True, dialogueContains=True))
    yield case('booth', 'regular ironman', define('booth') + [dict(op='interact', n=1, node='o', state=0)], 1, nodes=nodes, answers=dict(checkRestriction=True))
    yield case('booth', 'regular quick open', define('booth') + [dict(op='interact', n=1, node='o', state=0)], 1, nodes=nodes,
               constants=dict(BANK_BOOTH_QUICK_OPEN=True))
    yield case('booth', 'regular banker', define('booth') + [dict(op='interact', n=1, node='o', state=0)], 1, nodes=nodes,
               answers=dict(north, dialogueContains=True))
    yield case('booth', 'regular banker no dialogue', define('booth') + [dict(op='interact', n=1, node='o', state=0)], 1, nodes=nodes, answers=north)
    yield case('booth', 'regular other npc first', define('booth') + [dict(op='interact', n=1, node='o', state=0)], nodes=nodes,
               answers={'findNpc:3209,3220,2': 'm', 'findNpc:3207,3218,0': 'k'})
    yield case('booth', 'regular square', define('booth') + [dict(op='interact', n=1, node='o', state=0)], 1, nodes=nodes,
               answers={'findNpc:3210,3222,0': 'k', 'dialogueContains': True})
    yield case('booth', 'regular square other', define('booth') + [dict(op='interact', n=1, node='o', state=0)], nodes=nodes,
               answers={'findNpc:3206,3218,0': 'm', 'findNpc:3207,3218,0': 'k'})
    yield case('booth', 'regular nobody', define('booth') + [dict(op='interact', n=1, node='o', state=0)], 1, nodes=nodes)
    yield case('booth', 'collect', define('booth') + [dict(op='interact', n=2, node='o', state=0)], 1, nodes=nodes)
    yield case('booth', 'collect lunar', define('booth') + [dict(op='interact', n=2, node='o', state=0)], 1, nodes=nodes,
               answers=dict(north, lunarRestriction=True))
    use = lambda used: dict(op='interact', n=3, used=used, **{'with': 'o'})
    yield case('booth', 'note no option', define('booth') + [use('i')], 1, nodes=nodes, answers=dict(hasOption=False))
    yield case('booth', 'note ultimate', define('booth') + [use('i')], 1, nodes=nodes, answers=dict(checkRestriction=True))
    yield case('booth', 'note ultimate allowed', define('booth') + [use('i')], nodes=nodes, answers=dict(checkRestriction=True, amountInInventory=5),
               constants=dict(BANK_BOOTH_NOTE_UIM=False), **{'values': {}})
    yield case('booth', 'note lunar', define('booth') + [use('i')], 1, nodes=nodes, answers=dict(lunarRestriction=True))
    yield case('booth', 'note unnoted', define('booth') + [use('i')], 1, nodes=nodes, answers={'noteChange:i': 1512, 'amountInInventory': 5})
    yield case('booth', 'note not removed', define('booth') + [use('i')], nodes=nodes, answers={'noteChange:i': 1512, 'amountInInventory': 5, 'removeItem': False})
    yield case('booth', 'note cannot', define('booth') + [use('i')], 1, nodes=nodes)
    for amount, free in [(30, 10), (30, 29), (30, 30), (30, 31), (30, 28), (5, 0), (1, 0)]:
        yield case('booth', f'unnote {amount} {free}', define('booth') + [use('x')], 1 if (amount, free) == (30, 10) else None,
                   nodes=dict(nodes, x=dict(kind='item', id=1512, amount=amount)),
                   answers={'noteChange:x': 1511, 'isUnnoted:x': False, 'freeSlots': free})
    yield case('booth', 'note scenery used', define('booth') + [dict(op='interact', n=3, used='o', **{'with': 'o'})], nodes=nodes, answers={'noteChange:o': 3})
    yield case('booth', 'note npc used', define('booth') + [dict(op='interact', n=3, used='m', **{'with': 'o'})], nodes=nodes)
    yield case('chest', 'open', define('chest') + [dict(op='interact', n=0, node='o', state=1)], 1, nodes=nodes)
    yield case('boxes', 'open', define('boxes') + [dict(op='interact', n=0, node='o', state=0), task(0, iface=11)], 1, nodes=nodes,
               containers=dict(inventory=['i', None, 'x'] + [None] * 25))
    yield case('boxes', 'close event', define('boxes') + [dict(op='interact', n=0, node='o', state=0), task(0, iface=11)], 2, nodes=nodes)
    yield case('boxes', 'ultimate', define('boxes') + [dict(op='interact', n=0, node='o', state=0)], 1, nodes=nodes, answers=dict(checkRestriction=True))
    yield case('boxes', 'null component', define('boxes') + [dict(op='interact', n=0, node='o', state=0)], nodes=nodes, answers=dict(openNull=True))
    # LookAtOptionPlugin
    sign = dict(kind='scenery', id=18877, name='Wooden SIGNS', location=[2400, 3400, 0])
    yield case('lookat', 'look', define('lookat') + [dict(op='handler', via='sceneryId:18877:option:look at', node='s', option='look at')], 1, nodes=dict(s=sign))
    yield case('lookat', 'last id', define('lookat') + [dict(op='handler', via='sceneryId:18900:option:look at', node='s', option='look at')], nodes=dict(s=sign))
    yield case('lookat', 'unregistered', define('lookat') + [dict(op='handler', via='sceneryId:18901:option:look at', node='s', option='look at')], nodes=dict(s=sign))
    yield case('lookat', 'no name', define('lookat') + [dict(op='handler', via='sceneryId:18877:option:look at', node='s', option='look at')],
               nodes=dict(s=dict(kind='scenery', id=18877)))
    yield case('lookat', 'props', define('lookat') + [dict(op='props', via='sceneryId:18877:option:look at', node='s')], nodes=dict(s=sign))
    # SearchOptionPlugin
    sack = lambda name, i=14743: dict(s=dict(kind='scenery', id=i, name=name))
    search = define('search') + [dict(op='handler', via='scenery:search', node='s', option='search')]
    yield case('search', 'bookcase', search, 1, nodes=sack('Bookcase', 1))
    yield case('search', 'knife', search, 1, nodes=sack('Sacks'))
    yield case('search', 'has knife', search, 1, nodes=sack('Sacks'), answers=dict(containItems=True))
    yield case('search', 'in combat', search, 1, nodes=sack('Sacks'), answers={'inCombat:p': True})
    yield case('search', 'other', search, 1, nodes=sack('CRATE', 1))
    yield case('search', 'bookcase id', search, nodes=sack('Bookcase'))
    yield case('search', 'no name', search, nodes=dict(s=dict(kind='scenery', id=1)))
    yield case('search', 'props', define('search') + [dict(op='props', via='scenery:search', node='s')], nodes=sack('Sacks'))



def npcs():
    peer = dict(e=dict(kind='npc', id=1288, name='Peer the Seer'))
    yield case('npcdeposit', 'boots', define('npcdeposit') + [dict(op='interact', n=0, node='e')], 1, nodes=peer, answers=dict(anyInEquipment=True))
    yield case('npcdeposit', 'no boots', define('npcdeposit') + [dict(op='interact', n=0, node='e')], 1, nodes=peer)
    dummy = dict(d=dict(kind='scenery', id=2038, name='Dummy'))
    attack = lambda via='sceneryId:2038:option:attack': define('dummy') + [dict(op='handler', via=via, node='d', option='attack')]
    yield case('dummy', 'trained', attack(), 1, nodes=dummy, answers=dict(combatLevel=8))
    for style in [0, 1, 2, 3, 4]:
        yield case('dummy', f'style {style}', attack(), 1 if style in (0, 2) else None, nodes=dummy, answers=dict(attackStyle=style, combatLevel=7))
    yield case('dummy', 'other id', attack('sceneryId:23921:option:attack'), nodes=dummy)
    yield case('dummy', 'props', define('dummy') + [dict(op='props', via='sceneryId:823:option:attack', node='d')], nodes=dummy)


def reentry():
    # Hooks change the world while a listener runs: later reads see the change.
    yield case('reentry', 'logout zone then combat', define('logout') + [btn(182, 6)],
               hooks=[dict(event='canLogout', actions=[dict(answers={'inCombat:p': True})])])
    yield case('reentry', 'stats attribute removed during read', define('stats') + [btn(320, 125)],
               attributes=dict(p={'levelup:0': True}), hooks=[dict(event='apiGetAttribute', actions=[dict(attributes=dict(p={'levelup:0': None}))])])
    yield case('reentry', 'clan renamed between reads', define('clan') + [btn(590, 22, 155), task(0, value='x')],
               hooks=[dict(event='clanName', actions=[dict(values=dict(clanName='Chat disabled'))])])
    yield case('reentry', 'itemselect keepalive set by callback', define('itemselect') + [btn(12, 0, 155, 1)],
               attributes=dict(p={'itemselect-callback': {'callback': 'pick'}}),
               hooks=[dict(event='callback', actions=[dict(attributes=dict(p={'itemselect-keepalive': True}))])])
    yield case('reentry', 'charge failing dialogue', define('charge') + [btn(619, 28)], answers=dict(amountInInventory=0),
               hooks=[dict(event='npcDialogue', actions=[dict(op='fail')])])


def special_ladders():
    import importlib.util
    spec = importlib.util.spec_from_file_location('port_special_ladders', pathlib.Path(__file__).resolve().parent / 'port_special_ladders.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.entries(module.SOURCE.read_bytes().replace(b'\r', b'').decode())


def climbing():
    def ladder(name='Ladder', at=(3200, 3200, 0), **extra):
        spec = dict(kind='scenery', id=1747, name=name, location=list(at), rotation=0, size=[1, 1], walkingFlag=0, options=['Climb-up'])
        spec.update(extra)
        return spec
    up = lambda node='l', option='climb-up', via=None: dict(op='handler', via=via or 'scenery:' + (option or 'climb'), node=node, option=option)
    base = define('ladder')
    for k, (name, at, _, _) in enumerate(special_ladders()):
        yield case('climb', f'special {name}', base + [up(), task(0)], 1 if k in (0, 4) else None, nodes=dict(l=ladder(at=at)))
    first = special_ladders()[0][1]
    yield case('climb', 'special stairs', base + [up(), task(0)], nodes=dict(l=ladder('Staircase', at=first)))
    above = {'object:3200,3200,1': 'u'}
    upper = dict(l=ladder(), u=ladder(at=(3200, 3200, 1), options=['Climb-down']))
    yield case('climb', 'up', base + [up(), task(0)], 1, nodes=upper, answers=above)
    yield case('climb', 'up task', base + [up(), task(0)], 2, nodes=upper, answers=above)
    yield case('climb', 'walk up', base + [up(option='walk-up'), task(0)], nodes=upper, answers=above)
    yield case('climb', 'up blocked south', base + [up(), task(0)], nodes=upper, answers=dict(above, **{'clip:3200,3199,1': 0x100000}))
    yield case('climb', 'up blocked other bit', base + [up()], nodes=upper, answers=dict(above, **{'clip:3200,3199,1': 0x200000}))
    yield case('climb', 'up not permitted', base + [up()], nodes=upper, answers=dict(above, **{'teleport:3200,3199,1': False}))
    nowhere = {f'teleport:{x},{y},1': False for x, y in [(3200, 3199), (3199, 3200), (3200, 3201), (3201, 3200)]}
    yield case('climb', 'up nowhere', base + [up()], 1, nodes=upper, answers=dict(above, **nowhere))
    for flag in [1, 2, 4, 7, 8, 11, 13, 14, 15, 16, -1]:
        for rotation in [0, 1, 2, 3]:
            nodes = dict(l=ladder(), u=ladder(at=(3200, 3200, 1), options=['Climb-down'], walkingFlag=flag, rotation=rotation))
            yield case('climb', f'walking flag {flag} rotation {rotation}', base + [up()], nodes=nodes, answers=above)
    for size, rotation in [([2, 1], 0), ([2, 1], 1), ([1, 3], 2), ([3, 2], 3), ([2, 2], 5), ([2, 2], -1)]:
        nodes = dict(l=ladder(), u=ladder(at=(3200, 3200, 1), options=['Climb-down'], size=size, rotation=rotation))
        yield case('climb', f'size {size} rotation {rotation}', base + [up()], nodes=nodes,
                   answers=dict(above, **{'teleport:3200,3199,1': False, 'teleport:3201,3199,1': False}))
    # A negative rotation still swaps the sizes (Java's -1 % 2), and the walking flag shifted by it can point east.
    for flag, rotation in [(256, -1), (256, -3), (512, -2)]:
        nodes = dict(l=ladder(), u=ladder(at=(3200, 3200, 1), options=['Climb-down'], size=[2, 1], walkingFlag=flag, rotation=rotation))
        yield case('climb', f'negative rotation {flag} {rotation}', base + [up()], nodes=nodes,
                   answers=dict(above, **{'teleport:3201,3200,1': False}))
    down = dict(l=ladder('Trapdoor'), d=ladder(at=(3200, 9600, 0), options=['Climb-up']))
    yield case('climb', 'down trapdoor basement', base + [up(option='climb-down'), task(0)], nodes=down, answers={'object:3200,9600,0': 'd'})
    yield case('climb', 'down plane below', base + [up(option='walk-down')], nodes=dict(l=ladder(at=(3200, 3200, 1)), d=ladder(options=['Climb-up'])),
               answers={'object:3200,3200,0': 'd'})
    yield case('climb', 'down find', base + [up(option='climb-down')], nodes=dict(l=ladder(), f=ladder(at=(3202, 3197, -1), options=[None, 'Climb-up'])),
               answers={'object:3202,3197,-1': 'f', 'object:3199,3198,-1': 'x'} , **{})
    yield case('climb', 'find trapdoor', base + [up()], nodes=dict(l=ladder(), f=ladder('Trapdoor', at=(3196, 3204, 1), options=[])),
               answers={'object:3196,3204,1': 'f'})
    yield case('climb', 'same name stack', base + [up()], nodes=dict(l=ladder(), x=ladder(at=(3200, 3200, 1), options=['Look']),
                                                                     v=ladder('Rock', at=(3200, 3200, 2), options=[])),
               answers={'object:3200,3200,1': 'x', 'object:3200,3200,2': 'v'})
    yield case('climb', 'same name empty', base + [up()], nodes=dict(l=ladder(), x=ladder(at=(3200, 3200, 1), options=['Look'])),
               answers={'object:3200,3200,1': 'x'})
    yield case('climb', 'other name', base + [up()], nodes=dict(l=ladder(), x=ladder('Crate', at=(3200, 3200, 1), options=['Look'])),
               answers={'object:3200,3200,1': 'x', 'object:3200,-3200,0': 'u', 'object:3200,3200,2': 'x'}, **{})
    both = dict(l=ladder(), u=ladder(at=(3200, 3200, 1), options=['Climb-down']), d=ladder(at=(3200, 9600, 0), options=['Climb-up']))
    both_answers = {'object:3200,3200,1': 'u', 'object:3200,9600,0': 'd'}
    dialogue = base + [up(option='climb'), dict(op='dialogue', n=0, button=1), task(1), dict(op='dialogue', n=0, button=2),
                       dict(op='dialogue', n=0, button=3)]
    yield case('climb', 'climb dialogue', dialogue, 1, nodes=both, answers=both_answers)
    yield case('climb', 'climb dialogue up', dialogue, 2, nodes=both, answers=both_answers)
    yield case('climb', 'climb dialogue task', dialogue, 3, nodes=both, answers=both_answers)
    yield case('climb', 'climb up only', base + [up(option='climb'), task(0)], nodes=upper, answers=above)
    yield case('climb', 'climb down only', base + [up(option='climb'), task(0)], nodes=down, answers={'object:3200,9600,0': 'd'})
    yield case('climb', 'climb neither', base + [up(option='climb')], nodes=dict(l=ladder()))
    yield case('climb', 'unknown option', base + [up(option='climb-sideways', via='scenery:climb')], 1, nodes=dict(l=ladder()))
    yield case('climb', 'null option', base + [dict(op='handler', via='scenery:climb', node='l')], nodes=dict(l=ladder()))
    yield case('climb', 'null name', base + [up()], nodes=dict(l=dict(kind='scenery', id=1, location=[3200, 3200, 0])))
    yield case('climb', 'null options', base + [up()], nodes=dict(l=ladder(), u=ladder(at=(3200, 3200, 1), options=None)), answers=above)
    yield case('climb', 'null name above', base + [up()], nodes=dict(l=ladder(), u=dict(kind='scenery', id=2, location=[3200, 3200, 1], options=['Look'])),
               answers=above)
    yield case('climb', 'destination', base + [dict(op='destination', via='scenery:climb-up', node='u')], 1, nodes=upper)
    yield case('climb', 'destination rotated', base + [dict(op='destination', via='scenery:climb-up', node='u')], nodes=dict(
        u=ladder(at=(3200, 3200, 1), rotation=3, size=[1, 2])))
    yield case('climb', 'props', base + [dict(op='props', via='scenery:climb-up', node='l')], nodes=dict(l=ladder()))
    yield case('climb', 'not scenery', base + [up(node='n', option='climb')], nodes=dict(n=N))
    yield case('climb', 'destination not scenery', base + [dict(op='destination', via='scenery:climb-up', node='n')], nodes=dict(n=N))
    yield case('climb', 'destination null', base + [dict(op='destination', via='scenery:climb-up')])


def cases():
    yield from registration()
    yield from player_options()
    yield from login_plugin()
    yield from tabs()
    yield from interfaces()
    yield from scenery()
    yield from npcs()
    yield from climbing()
    yield from reentry()


def injects(row):
    return any(any(a.get('op') == 'fail' for a in h.get('actions', [])) for h in row.get('hooks', []))


def groups(cases):
    out = {}
    for case_ in cases:
        out[case_['group']] = out.get(case_['group'], 0) + 1
    return dict(sorted(out.items()))