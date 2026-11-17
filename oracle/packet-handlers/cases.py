"""The packet-handlers corpus: each case is a world (nodes, interactions, containers, repository, answers) and operations.

`cases()` yields (case, sweep): when sweep is an operation index, generate.py adds one case per call recorded by that
operation, failing that call. Answers are values or lists taken in order (the last repeats).
"""
import copy
import itertools

NEEDED_EVENTS = [
    'get', 'replace', 'insert', 'remove', 'add', 'freeSlot', 'refresh', 'maximumAdd', 'itemsInTab', 'tabByItemSlot',
    'increaseTabStartSlots', 'decreaseTabStartSlots', 'setTabConfigurations', 'npc', 'player', 'shown', 'child', 'object',
    'objectAt', 'ground', 'interaction', 'option', 'handle', 'invalid', 'listeners', 'listenersUse', 'listenersEquip',
    'pluginNpc', 'pluginUse', 'zoneInteract', 'zoneUseWith', 'interactionLocked', 'movementLocked', 'equipmentLocked',
    'scripts', 'debug', 'message', 'packetMessage', 'sendMessage', 'minimap', 'sendString', 'clearPulses', 'submit',
    'movement', 'run', 'unhandled', 'closeInterface', 'hasChatbox', 'closeChatbox', 'attribute', 'dispatchUse',
    'dispatchEquip', 'dispatchUnequip', 'faceLocation', 'face', 'useHandle', 'handlerHandle', 'log', 'spellBook',
    'spellListeners', 'castSpell', 'hasFamiliar', 'familiarSpecial', 'itemDefinition', 'sceneryDefinition', 'npcDefinition',
    'varbit', 'replay', 'setPlaying', 'fireEvent', 'equipAdd', 'registerGlove', 'dialogueClose', 'audio', 'clearAutocast',
    'selectAutoSpell', 'openAutocastSelect', 'isSlayerEq', 'updateSlayerFlags', 'usedOption', 'pet', 'summon', 'grave',
    'closeAll', 'queueScript', 'stopExecuting', 'monitor', 'dropItem', 'groundCreate', 'setAttribute', 'save',
    'destroyDialogue', 'addDialogueAction', 'removeItem', 'on',
]


def op(name, **kw):
    return dict(op=name, **kw)


def world():
    return dict(
        nodes={
            'sword': dict(kind='item', id=1277, index=0, name='Bronze sword', interaction='i_sword'),
            'coins': dict(kind='item', id=995, index=1, name='Coins', amount=100, interaction='i_coins'),
            'shield': dict(kind='item', id=1171, index=2, name='Wooden shield', interaction='i_shield'),
            'worn': dict(kind='item', id=1205, index=3, name='Bronze dagger', interaction='i_worn'),
            'banked': dict(kind='item', id=440, index=0, name='Iron ore', interaction='i_banked'),
            'goblin': dict(kind='npc', id=100, index=5, interaction='i_goblin', spawn=[3222, 3219, 0], text='NPC goblin'),
            'morph': dict(kind='npc', id=101, index=5, interaction='i_morph', text='NPC morph'),
            'friend': dict(kind='player', id=7, index=7, interaction='i_friend', text='Player friend'),
            'tree': dict(kind='scenery', id=1276, location=[3223, 3218, 0], interaction='i_tree', text='Scenery tree'),
            'stump': dict(kind='scenery', id=1342, location=[3223, 3218, 0], text='Scenery stump'),
            'loot': dict(kind='ground', id=526, location=[3221, 3218, 0], name='Bones', text='GroundItem loot'),
        },
        interactions={
            'i_sword': dict(owner='sword', options=[None, dict(name='Wield', handler=1), None, None, dict(name='Drop')]),
            'i_coins': dict(owner='coins', options=[None, None, None, None, dict(name='Drop')]),
            'i_shield': dict(owner='shield', options=[None, dict(name='Wear', handler=2), None, None, dict(name='Drop')]),
            'i_worn': dict(owner='worn', options=[dict(name='Remove')]),
            'i_banked': dict(owner='banked', options=[dict(name='Withdraw-1')]),
            'i_goblin': dict(owner='goblin', options=[dict(name='Talk-to'), dict(name='Attack'), None, None, None]),
            'i_morph': dict(owner='morph', options=[dict(name='Talk-to'), None, dict(name='Pickpocket'), None, None]),
            'i_friend': dict(owner='friend', options=[dict(name='Follow'), dict(name='Trade with'), None, None, None, None, None, None]),
            'i_tree': dict(owner='tree', options=[dict(name='Chop down')]),
        },
        containers={'inventory': dict(slots={'0': 'sword', '1': 'coins', '2': 'shield'}),
                    'equipment': dict(slots={'3': 'worn'}),
                    'bank': dict(slots={'0': 'banked'})},
        players={'7': 'friend'},
        npcs={'5': 'goblin'},
        object='tree', objectAt='tree', ground='loot',
        values=dict(ticks=100),
        ops=[],
    )


def case(ops, **changes):
    w = world()
    for key, value in changes.items():
        if isinstance(value, dict) and isinstance(w.get(key), dict):
            merged = copy.deepcopy(w[key])
            for k, v in value.items():
                if isinstance(v, dict) and isinstance(merged.get(k), dict):
                    merged[k] = {**merged[k], **v}
                else:
                    merged[k] = v
            w[key] = merged
        else:
            w[key] = value
    w['ops'] = ops
    return w


def injects(row):
    """Whether a case fails a call on purpose through a hook."""
    return any(a.get('op') == 'fail' for h in row.get('hooks', []) for a in h['actions'])


def item_actions():
    out = []
    for iface, slot, item in itertools.product([149, 762, 667, 548], [-1, 0, 1, 3, 27, 28], [1277, 995, 1205, 440, 0]):
        out.append((case([op('item_action', opt=1, item=item, slot=slot, iface=iface, child=0)]), None))
    for opt in [-1, 0, 1, 4, 5, 8]:
        for locked in [False, True]:
            out.append((case([op('item_action', opt=opt, item=1277, slot=0, iface=149, child=0), op('pulse', n=0)],
                             flags=dict(interactionLocked=locked)), None))
    # The pulse body: locks and zone at run time, listeners, handlers, names.
    for locked, zone, listeners, handler, handled in itertools.product([False, True], [False, True], [False, True], [None, 1, 2], [False, True]):
        options = [None, dict(name='Wield', handler=handler), None, None, dict(name='Drop')]
        out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0),
                          op('set', flags=dict(interactionLocked=locked)), op('pulse', n=0)],
                         interactions=dict(i_sword=dict(owner='sword', options=options)),
                         answers={'zoneInteract': zone, 'listeners': listeners, 'handler:1': handled, 'handler:2': handled}), None))
    for name in [None, '', 'WIELD', 'İstanbul', 'drop']:
        for handler in [None, 1]:
            out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0), op('pulse', n=0)],
                             interactions=dict(i_sword=dict(owner='sword', options=[None, dict(name=name, handler=handler)])),
                             answers={'handler:1': True}), None))
    # Missing interaction, a bank and an equipment item, and the owner of a replaced interaction.
    out.append((case([op('item_action', opt=0, item=1277, slot=0, iface=149, child=0)], nodes=dict(sword=dict(interaction=None))), None))
    out.append((case([op('item_action', opt=0, item=440, slot=0, iface=762, child=0), op('pulse', n=0)]), None))
    out.append((case([op('item_action', opt=0, item=1205, slot=3, iface=667, child=0), op('pulse', n=0)]), None))
    out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0), op('pulse', n=0)],
                     hooks=[dict(event='interactionLocked', at=1, actions=[op('set', nodes=dict(sword=dict(interaction='i_shield', id=1171)))])]), None))
    out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0), op('pulse', n=0)],
                     hooks=[dict(event='option', at=1, actions=[op('set', interactions=dict(i_sword=dict(owner='sword', options=[None, dict(name='Replaced', handler=2)])))])]), None))
    out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0), op('pulse', n=0)],
                     hooks=[dict(event='listeners', at=1, actions=[op('fail')])]), None))
    out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0), op('pulse', n=0)],
                     hooks=[dict(event='handlerHandle', at=1, actions=[op('set', interactions=dict(i_sword=dict(owner='sword', options=[None, None])))])],
                     answers={'handler:1': False}), None))
    # Two items in one tick: two pulses, run in order.
    out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0), op('item_action', opt=1, item=1171, slot=2, iface=149, child=0),
                      op('pulse', n=1), op('pulse', n=0)], answers={'handler:2': [True], 'handler:1': [False]}), None))
    # Sweeps: the action and its pulse.
    out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0), op('pulse', n=0)], answers={'handler:1': False}), 0))
    out.append((case([op('item_action', opt=1, item=1277, slot=0, iface=149, child=0), op('pulse', n=0)], answers={'handler:1': False}), 1))
    return out


def npc_actions():
    out = []
    for index, opt in itertools.product([-2147483648, -1, 0, 1, 5, 31999, 32000, 2147483647], [-1, 0, 1, 2, 5]):
        out.append((case([op('npc_action', opt=opt, npc=index)]), None))
    for shown, listeners, plugin, spawn, config in itertools.product([None, 'morph'], [False, True], [False, True], [None, [3200, 3201, 1]], [-1, 4]):
        out.append((case([op('npc_action', opt=0, npc=5)], nodes=dict(goblin=dict(shown=shown, spawn=spawn, configFile=config)),
                         answers=dict(listeners=listeners, pluginNpc=plugin)), None))
    out.append((case([op('npc_action', opt=2, npc=5)], nodes=dict(goblin=dict(shown='morph')), answers=dict(listeners=[False, True])), None))
    out.append((case([op('npc_action', opt=1, npc=5)], nodes=dict(goblin=dict(shown='morph'))), None))
    out.append((case([op('npc_action', opt=0, npc=5)], nodes=dict(goblin=dict(interaction=None))), None))
    out.append((case([op('npc_action', opt=0, npc=5)], nodes=dict(goblin=dict(shown='morph'), morph=dict(id=100))), None))
    out.append((case([op('npc_action', opt=0, npc=5)], interactions=dict(i_goblin=dict(owner='goblin', options=[dict(name=None, index=9)]))), None))
    out.append((case([op('npc_action', opt=0, npc=6)], npcs={'6': 'morph'}), None))
    out.append((case([op('npc_action', opt=0, npc=5)], values=dict(maxNpcs=5)), None))
    out.append((case([op('set', values=dict(maxNpcs=10)), op('npc_action', opt=0, npc=5), op('npc_action', opt=0, npc=12)], values=dict(maxNpcs=40000)), None))
    out.append((case([op('npc_action', opt=0, npc=5)], nodes=dict(goblin=dict(shown='morph')),
                     hooks=[dict(event='debug', at=2, actions=[op('set', nodes=dict(morph=dict(id=100), goblin=dict(id=77)))])]), None))
    out.append((case([op('npc_action', opt=0, npc=5)],
                     hooks=[dict(event='pluginNpc', actions=[op('set', nodes=dict(goblin=dict(interaction='i_morph')))])]), None))
    out.append((case([op('npc_action', opt=0, npc=5)], nodes=dict(goblin=dict(shown='morph'))), 0))
    return out


def player_actions():
    out = []
    for index, opt in itertools.product([-1, 0, 1, 7, 1999, 2000, 2147483647], [-1, 0, 1, 2, 8]):
        out.append((case([op('player_action', opt=opt, other=index)]), None))
    for active, listeners, name in itertools.product([False, True], [False, True], ['Trade with', 'FOLLOW', 'Ünïcode', None]):
        out.append((case([op('player_action', opt=1, other=7)], nodes=dict(friend=dict(active=active)),
                         interactions=dict(i_friend=dict(owner='friend', options=[None, dict(name=name)])),
                         answers=dict(listeners=listeners)), None))
    out.append((case([op('player_action', opt=0, other=8)]), None))
    out.append((case([op('player_action', opt=0, other=7)], nodes=dict(friend=dict(interaction=None))), None))
    out.append((case([op('player_action', opt=0, other=7)], values=dict(maxPlayers=7)), None))
    out.append((case([op('player_action', opt=0, other=7)],
                     hooks=[dict(event='listeners', actions=[op('set', nodes=dict(friend=dict(interaction='i_goblin')))])]), None))
    out.append((case([op('player_action', opt=1, other=7)]), 0))
    return out


def use_withs():
    out = []
    handler = lambda ref, id, type, cls='FirstUse', allowed=(), dynamic=False: op('use_register', ref=ref, id=id, type=type, **{'class': cls},
                                                                                allowed=None if allowed is None else list(allowed), dynamic=dynamic)
    # Lookups and validation for each packet.
    for iface, slot, item in itertools.product([149, 762, 548], [0, 1, 40], [1277, 0]):
        out.append((case([op('use_npc', item=item, npc=5, iface=iface, slot=slot)]), None))
        out.append((case([op('use_player', item=item, other=7, iface=iface, slot=slot)]), None))
        out.append((case([op('use_ground', used=item, **{'with': 526}, iface=iface, slot=slot, x=3221, y=3218)]), None))
    for npc in [-1, 0, 5, 6, 32000]:
        out.append((case([op('use_npc', item=1277, npc=npc, iface=149, slot=0)]), None))
    for other in [-1, 0, 7, 8, 2000]:
        out.append((case([op('use_player', item=1277, other=other, iface=149, slot=0)]), None))
    for slot, scenery, found, child in itertools.product([0, 5], [1276, 1342], ['tree', None], [None, 'stump', 'tree']):
        out.append((case([op('use_scenery', item=1277, slot=slot, scenery=scenery, x=3223, y=3218)], object=found,
                         nodes=dict(tree=dict(child=child))), None))
    for used, with_, ui, wi, us, ws in [(1277, 995, 149, 149, 0, 1), (995, 1277, 149, 149, 1, 0), (1277, 1277, 149, 149, 0, 0),
                                       (1277, 440, 149, 762, 0, 0), (1277, 995, 548, 149, 0, 1), (1277, 995, 149, 548, 0, 1),
                                       (1277, 995, 149, 149, 5, 1), (1277, 995, 149, 149, 0, 5), (1, 995, 149, 149, 0, 1),
                                       (1277, 1, 149, 149, 0, 1), (1205, 1277, 667, 149, 3, 0)]:
        out.append((case([op('use_item', used=used, **{'with': with_}, usedSlot=us, withSlot=ws, usedIface=ui, withIface=wi), op('pulse', n=0)]), None))
    for found, with_ in itertools.product(['loot', None], [526, 527]):
        out.append((case([op('use_ground', used=1277, **{'with': with_}, iface=149, slot=0, x=3221, y=3218)], ground=found), None))
    # Locks, zone, listeners (node, then the shown NPC or scenery child), plugin.
    for il, ml, zone, lis, plugin in itertools.product([False, True], [False, True], [False, True], [[False], [True], [False, True]], [False, True]):
        out.append((case([op('use_npc', item=1277, npc=5, iface=149, slot=0)], nodes=dict(goblin=dict(shown='morph')),
                         flags=dict(interactionLocked=il, movementLocked=ml), answers=dict(zoneUseWith=zone, listenersUse=lis, pluginUse=plugin)), None))
    out.append((case([op('use_npc', item=1277, npc=5, iface=149, slot=0)], nodes=dict(goblin=dict(shown='morph'), morph=dict(id=100))), None))
    out.append((case([op('use_scenery', item=1277, slot=0, scenery=1276, x=3223, y=3218)], nodes=dict(tree=dict(child='stump')),
                     answers=dict(listenersUse=[False, True])), None))
    # UseWithHandler.run: no handler, then each lookup key and pulse body.
    targets = [
        op('use_npc', item=1277, npc=5, iface=149, slot=0),
        op('use_player', item=1277, other=7, iface=149, slot=0),
        op('use_scenery', item=1277, slot=0, scenery=1276, x=3223, y=3218),
        op('use_ground', used=1277, **{'with': 526}, iface=149, slot=0, x=3221, y=3218),
        op('use_item', used=1277, **{'with': 995}, usedSlot=0, withSlot=1, usedIface=149, withIface=149),
        op('use_item', used=995, **{'with': 1277}, usedSlot=1, withSlot=0, usedIface=149, withIface=149),
    ]
    for target in targets:
        out.append((case([target, op('pulse', n=0)]), None))
        out.append((case([target, op('pulse', n=0)], nodes=dict(goblin=dict(shown='morph'), tree=dict(child='stump'))), None))
    registrations = [
        [handler('a', 101, 1, allowed=[1277])],
        [handler('a', 100, 1, allowed=[1277]), handler('b', 101, 1, 'SecondUse', allowed=[1277])],
        [handler('a', 1276, 2, allowed=[995]), handler('b', 1342, 2, 'SecondUse', allowed=[1277])],
        [handler('a', 1277, 3, allowed=None), handler('b', 995, 3, 'SecondUse', allowed=[])],
        [handler('a', 1277, 0, allowed=[995]), handler('b', 1277, 0, 'SecondUse', allowed=[526])],
        [handler('a', 995, 0, allowed=[1]), handler('b', 995, 0, 'SecondUse', dynamic=True, allowed=None)],
        [handler('a', 526, 0, allowed=[1277])],
        [handler('a', 1277, 0, allowed=None)],
        [handler('a', 100, 1, allowed=None)],
        [handler('a', 100, 1, dynamic=True, allowed=None), handler('b', 100, 1, 'SecondUse', allowed=[1277])],
    ]
    for registration, target, answer in itertools.product(registrations, targets, [[False], [True], [False, True]]):
        out.append((case(registration + [target, op('pulse', n=0)], nodes=dict(goblin=dict(shown='morph'), tree=dict(child='stump')),
                         answers={'useHandle:a': answer, 'useHandle:b': [True]}), None))
    # addHandler's player-type array growth on a shared handler.
    out.append((case([handler('a', 1277, 3, allowed=[5]), handler('a', 995, 3, allowed=[5]), handler('a', 1171, 0, allowed=[5]),
                      op('use_player', item=995, other=7, iface=149, slot=1), op('pulse', n=0)], answers={'useHandle:a': True}), None))
    out.append((case([handler('a', 1277, 3, allowed=None), op('use_player', item=1277, other=7, iface=149, slot=0), op('pulse', n=0)],
                     answers={'useHandle:a': True}), None))
    # Registration while a pulse waits joins the live list.
    out.append((case([handler('a', 100, 1, allowed=[1]), op('use_npc', item=1277, npc=5, iface=149, slot=0),
                      handler('b', 100, 1, 'SecondUse', allowed=[1277]), op('pulse', n=0)], answers={'useHandle:b': True}), None))
    out.append((case([handler('a', 1277, 0, allowed=[1]), op('use_item', used=1277, **{'with': 995}, usedSlot=0, withSlot=1, usedIface=149, withIface=149),
                      handler('b', 1277, 0, 'SecondUse', allowed=[995]), op('pulse', n=0)], answers={'useHandle:b': True}), None))
    # Failures inside run are printed and swallowed; in the pulses they propagate.
    out.append((case([handler('a', 100, 1, allowed=[1277]), op('use_npc', item=1277, npc=5, iface=149, slot=0)],
                     hooks=[dict(event='closeInterface', actions=[op('fail')])]), None))
    out.append((case([handler('a', 100, 1, allowed=[1277]), op('use_npc', item=1277, npc=5, iface=149, slot=0), op('pulse', n=0)],
                     hooks=[dict(event='useHandle', actions=[op('fail')])]), None))
    # UseWithHandler.run called directly: a used node that is not an item fails the Item cast (swallowed).
    for used, with_ in [('friend', 'sword'), ('goblin', 'sword'), ('tree', 'loot'), ('sword', 'goblin'), ('sword', 'friend'), ('sword', 'tree'),
                        ('friend', 'goblin'), ('coins', 'sword'), ('loot', 'sword'), ('goblin', 'friend'), ('tree', 'friend'), ('loot', 'friend')]:
        for registration in [[], [handler('a', 1277, 0, allowed=[995]), handler('b', 100, 1, allowed=[1277]), handler('c', 1277, 3, allowed=[7]),
                                  handler('d', 1276, 2, allowed=[1277])]]:
            out.append((case(registration + [op('use_run', used=used, **{'with': with_}), op('pulse', n=0)], answers={'useHandle:a': True}), None))
    # Late ids: the flip and the lookups read the ids again.
    out.append((case([op('use_item', used=1277, **{'with': 995}, usedSlot=0, withSlot=1, usedIface=149, withIface=149), op('pulse', n=0)],
                     hooks=[dict(event='movementLocked', actions=[op('set', nodes=dict(sword=dict(id=1), coins=dict(id=2000)))])]), None))
    for target in targets:
        out.append((case([op('use_register', ref='a', id=100, type=1, **{'class': 'FirstUse'}, allowed=[1277], dynamic=False),
                          op('use_register', ref='b', id=1276, type=2, **{'class': 'FirstUse'}, allowed=[1277], dynamic=False),
                          op('use_register', ref='c', id=1277, type=3, **{'class': 'SecondUse'}, allowed=[], dynamic=False),
                          op('use_register', ref='d', id=1277, type=0, **{'class': 'SecondUse'}, allowed=[995, 526], dynamic=False),
                          target, op('pulse', n=0)], answers={'useHandle:a': False, 'useHandle:b': False, 'useHandle:c': False, 'useHandle:d': False}), 4))
        out.append((case([op('use_register', ref='a', id=100, type=1, **{'class': 'FirstUse'}, allowed=[1277], dynamic=False),
                          op('use_register', ref='b', id=1276, type=2, **{'class': 'FirstUse'}, allowed=[1277], dynamic=False),
                          op('use_register', ref='c', id=1277, type=3, **{'class': 'SecondUse'}, allowed=[], dynamic=False),
                          op('use_register', ref='d', id=1277, type=0, **{'class': 'SecondUse'}, allowed=[995, 526], dynamic=False),
                          target, op('pulse', n=0)], answers={'useHandle:a': False, 'useHandle:b': False, 'useHandle:c': False, 'useHandle:d': False}), 5))
    return out


def components():
    out = []
    targets = [
        op('comp_ground', iface=192, child=44, item=526, x=3221, y=3218),
        op('comp_player', other=7, iface=193, child=20),
        op('comp_scenery', iface=430, child=11, scenery=1276, x=3223, y=3218),
        op('comp_npc', iface=192, child=1, npc=5),
        op('comp_item', iface=192, child=33, item=1277, slot=0),
    ]
    for target, iface in itertools.product(targets, [192, 193, 430, 662, 149]):
        for book in [192, 193]:
            for familiar in [False, True]:
                out.append((case([dict(target, iface=iface)], values=dict(spellBook=book), flags=dict(familiar=familiar)), None))
    for delay in [None, 99, 100, 101, 'text']:
        for target in targets:
            out.append((case([dict(target, iface=192)], values=dict(spellBook=192), attributes={} if delay is None else {'magic:delay': delay}), None))
    misses = [
        (op('comp_ground', iface=192, child=44, item=526, x=3221, y=3218), dict(ground=None)),
        (op('comp_ground', iface=192, child=44, item=527, x=3221, y=3218), {}),
        (op('comp_player', other=8, iface=192, child=20), {}),
        (op('comp_player', other=0, iface=192, child=20), {}),
        (op('comp_scenery', iface=192, child=11, scenery=1276, x=3223, y=3218), dict(objectAt=None)),
        (op('comp_scenery', iface=192, child=11, scenery=1277, x=3223, y=3218), {}),
        (op('comp_npc', iface=192, child=1, npc=0), {}),
        (op('comp_npc', iface=192, child=1, npc=32000), {}),
        (op('comp_npc', iface=192, child=1, npc=6), {}),
        (op('comp_item', iface=192, child=33, item=1277, slot=1), {}),
        (op('comp_item', iface=192, child=33, item=1277, slot=40), {}),
    ]
    for target, change in misses:
        out.append((case([target], values=dict(spellBook=192), **change), None))
    out.append((case([op('comp_ground', iface=192, child=44, item=526, x=3221, y=3218)], values=dict(spellBook=192), nodes=dict(p=dict(location=None))), None))
    for target in targets:
        out.append((case([dict(target, iface=662)], flags=dict(familiar=True)), 0))
        out.append((case([dict(target, iface=192)], values=dict(spellBook=192)), 0))
    return out


def slot_switches():
    out = []
    bank = dict(bank=dict(capacity=20, slots={'0': 'banked', '1': 'coins', '2': 'shield', '5': 'sword'}, tabStart=[0, 2, 4, 4, 4, 4, 4, 4, 4, 4, 6]))
    for dst_child in [73, 41, 74, 39, 37, 35, 23, 21, 40, 0]:
        for src in [0, 1, 5, 19]:
            out.append((case([op('switch_multi', srcIface=762, src=src, dstIface=762, dstChild=dst_child, dst=3)], containers=bank,
                             answers=dict(itemsInTab=[2], tabByItemSlot=[1])), None))
    for second, remove in itertools.product([[0], [-1]], [True, False]):
        full = dict(bank=dict(capacity=4, slots={'0': 'banked', '1': 'coins', '2': 'shield', '3': 'sword'}))
        out.append((case([op('switch_multi', srcIface=762, src=1, dstIface=762, dstChild=41, dst=0)], containers=full,
                         answers={'remove:bank': remove}), None))
    out.append((case([op('switch_multi', srcIface=762, src=1, dstIface=762, dstChild=39, dst=0)], containers=bank, answers=dict(itemsInTab=[-5])), None))
    out.append((case([op('switch_multi', srcIface=762, src=3, dstIface=762, dstChild=39, dst=0)], containers=bank), None))
    for insert in [False, True]:
        for src, dst in [(0, 5), (5, 0), (0, 0), (1, 19), (-1, 0), (0, 20), (3, 4)]:
            out.append((case([op('switch_multi', srcIface=762, src=src, dstIface=762, dstChild=73, dst=dst)], containers=bank, flags=dict(insertItems=insert)), None))
            out.append((case([op('switch_single', iface=762, src=src, dst=dst, insert=insert)], containers=bank), None))
            out.append((case([op('switch_single', iface=149, src=src, dst=dst, insert=insert)]), None))
    for src, dst in [(0, 2), (2, 0), (0, 27), (27, 0), (0, 28), (3, 1), (-1, 2)]:
        out.append((case([op('switch_multi', srcIface=149, src=src, dstIface=149, dstChild=0, dst=dst)]), None))
        out.append((case([op('switch_multi', srcIface=149, src=src, dstIface=548, dstChild=0, dst=dst)]), None))
    for chatbox, close in itertools.product([False, True], [None, False, True, 'yes']):
        out.append((case([op('switch_single', iface=149, src=0, dst=2, insert=False)], flags=dict(chatbox=chatbox),
                         attributes={} if close is None else {'close_c_': close}), None))
        out.append((case([op('switch_single', iface=149, src=0, dst=2, insert=True)], flags=dict(chatbox=chatbox),
                         attributes={} if close is None else {'close_c_': close}), None))
    out.append((case([op('switch_single', iface=149, src=0, dst=1, insert=False)], flags=dict(chatbox=True)), 0))
    out.append((case([op('switch_multi', srcIface=762, src=1, dstIface=762, dstChild=39, dst=0)], containers=bank, answers=dict(itemsInTab=[2], tabByItemSlot=[1])), 0))
    out.append((case([op('switch_multi', srcIface=762, src=1, dstIface=762, dstChild=41, dst=0)],
                     containers=dict(bank=dict(capacity=2, slots={'0': 'banked', '1': 'coins'}))), 0))
    return out


def examines():
    out = []
    names = ['Bronze sword', 'apple', 'Egg', 'iron', 'Orb', 'umbrella', 'hat', 'Helmet', 'h', 'H', 'Hello', 'x', '', None, 'İnk', 'yew']
    for name, examine, handler, unnoted in itertools.product(names, [None, 'A fine blade.'], [None, 'Configured.', 12, True], [True, False]):
        defs = {'1277': dict(name=name, examine=examine, unnoted=unnoted, value=32, model=2604, handlers={} if handler is None else {'examine': handler})}
        out.append((case([op('examine_item', id=1277), op('examine_item', id=1277)], itemDefs=defs), None))
    out.append((case([op('examine_item', id=4)], missing=['item:4']), None))
    out.append((case([op('examine_item', id=5)]), None))
    for name, examine, handler, config, varbit in itertools.product(names, [None, 'Tall.'], [None, 'Oak.', 7], [-1, 0, 9], [None, [9], [9, None], [None, 9]]):
        defs = {'1276': dict(name=name, examine=examine, configFile=config, handlers={} if handler is None else {'examine': handler})}
        out.append((case([op('examine_scenery', id=1276), op('examine_scenery', id=1276)], sceneryDefs=defs,
                         answers={} if varbit is None else dict(varbit=varbit)), None))
    out.append((case([op('examine_scenery', id=2)], missing=['scenery:2']), None))
    for name, examine, handler, config in itertools.product(names, [None, 'Ugly.'], [None, 'Small.', 3, None], [-1, 12]):
        defs = {'100': dict(name=name, examine=examine, configFile=config, handlers={} if handler is None else {'examine': handler})}
        out.append((case([op('examine_npc', id=100)], npcDefs=defs), None))
    out.append((case([op('examine_npc', id=3)], missing=['npc:3']), None))
    for looping in [False, True]:
        out.append((case([op('track', track=76), op('track', track=-1)], flags=dict(looping=looping)), None))
    out.append((case([op('examine_item', id=1277)], itemDefs={'1277': dict(name='Bronze sword', unnoted=False)}), 0))
    out.append((case([op('examine_scenery', id=1276)], sceneryDefs={'1276': dict(name='Tree', configFile=3)}), 0))
    out.append((case([op('examine_npc', id=100)], npcDefs={'100': dict(name='Goblin', configFile=3)}), 0))
    out.append((case([op('track', track=1)], flags=dict(looping=True)), 0))
    return out


def equips():
    out = []
    plugin = {'equipment': {'plugin': 'gear'}}
    for slot, current in itertools.product([None, -1, 3, 5, 14, 'x'], [None, 'worn', 'sword']):
        handlers = {} if slot is None else {'equipment_slot': slot}
        place = slot if isinstance(slot, int) and 0 <= slot < 14 else 3
        slots = {} if current is None else {str(place): current}
        out.append((case([op('equip', node='sword')], itemDefs={'1277': dict(handlers=handlers)}, containers=dict(equipment=dict(slots=slots))), None))
    for name in ['Goblin mail', 'GOBLIN MAIL top', 'goblin', None, 'Bronze sword']:
        out.append((case([op('equip', node='sword')], nodes=dict(sword=dict(name=name)), itemDefs={'1277': dict(handlers={'equipment_slot': 3})}), None))
    for fired, equip, unequip in itertools.product([None, True, False, 'other'], [False, True], [False, True]):
        out.append((case([op('equip', node='sword')], itemDefs={'1277': dict(handlers={'equipment_slot': 3, **plugin})},
                         answers={'fireEvent:gear': fired, 'listenersEquip:true': equip, 'listenersEquip:false': unequip}), None))
    for has, locked, message, added in itertools.product([False, True], [False, True], [None, 'You cannot do that now.'], [False, True]):
        out.append((case([op('equip', node='sword')], itemDefs={'1277': dict(handlers={'equipment_slot': 3})},
                         flags=dict(equipmentLock=has, equipmentLocked=locked), values=dict(lockMessage=message), answers=dict(equipAdd=added)), None))
    for item, slot, autocast, weapon, slayer, audio in itertools.product([1277, 13845, 13857, 13858], [3, 5], [False, True], [False, True], [False, True], [None, 2244, 'loud']):
        handlers = {'equipment_slot': slot}
        if audio is not None:
            handlers['equip_audio'] = audio
        out.append((case([op('equip', node='sword')], nodes=dict(sword=dict(id=item, index=0, definition=1277)), itemDefs={'1277': dict(handlers=handlers)},
                         flags=dict(autocast=autocast, noWeaponInterface=not weapon), answers=dict(isSlayerEq=slayer)), None))
    out.append((case([op('equip', node='goblin')]), None))
    out.append((case([op('equip', node='loot')], itemDefs={'526': dict(handlers={'equipment_slot': 3})}), None))
    # unequip.
    for slot in [-1, 0, 3, 5, 13, 14]:
        for weapon_slot_item in ['worn', None]:
            out.append((case([op('unequip', slot=slot, item=1205)], containers=dict(equipment=dict(slots={} if weapon_slot_item is None else {str(max(0, min(slot, 13))): weapon_slot_item})),
                             itemDefs={'1205': dict(handlers={'equip_audio': 2240})}), None))
    for has, locked, message in itertools.product([False, True], [False, True], [None, 'Locked.']):
        out.append((case([op('unequip', slot=3, item=1205)], flags=dict(equipmentLock=has, equipmentLocked=locked), values=dict(lockMessage=message)), None))
    for maximum, amount in [(0, 1), (1, 1), (5, 10), (10, 10), (-1, 0)]:
        out.append((case([op('unequip', slot=3, item=1205)], nodes=dict(worn=dict(amount=amount)), answers=dict(maximumAdd=maximum)), None))
    for fired, listeners, removed, slayer in itertools.product([None, True, False, 'other'], [False, True], [False, True], [False, True]):
        out.append((case([op('unequip', slot=3, item=1205)], itemDefs={'1205': dict(handlers=plugin)},
                         answers={'fireEvent:gear': fired, 'listenersEquip:false': listeners, 'remove:equipment': removed, 'isSlayerEq': slayer}), None))
    out.append((case([op('unequip', slot=3, item=999)]), None))
    # Registration and the registered handler.
    out.append((case([op('equip_define'), op('invoke', n=0, node='sword'), op('invoke', n=0, node='goblin')], itemDefs={'1277': dict(handlers={'equipment_slot': 3})}), None))
    out.append((case([op('equip', node='sword')], itemDefs={'1277': dict(handlers={'equipment_slot': 3, **plugin})},
                     answers={'fireEvent:gear': True, 'isSlayerEq': True}, flags=dict(autocast=True, equipmentLock=True),
                     containers=dict(equipment=dict(slots={'3': 'worn'}))), 0))
    out.append((case([op('unequip', slot=3, item=1205)], itemDefs={'1205': dict(handlers=plugin)}, answers={'fireEvent:gear': True, 'isSlayerEq': True},
                     flags=dict(equipmentLock=True)), 0))
    return out


def drops():
    out = []
    for option, destroy in itertools.product(['drop', 'destroy', 'dissolve', 'Drop', 'INVALID', None], [None, True, False, 'yes', 'null']):
        handlers = {} if destroy is None else {'destroy': None if destroy == 'null' else destroy}
        values = {} if option is None else dict(usedOption=option)
        out.append((case([op('drop', node='sword'), op('dialogue', n=0, button=3)], values=values, itemDefs={'1277': dict(handlers=handlers)}), None))
    for pet, grave, lock in itertools.product([False, True], [False, True], [False, True]):
        out.append((case([op('drop', node='sword')], values=dict(usedOption='drop'), answers=dict(pet=pet, grave=grave), flags=dict(equipmentLock=lock)), None))
    # The queued script.
    scripts = [
        ({}, {}),
        (dict(containers=dict(inventory=dict(slots={'0': 'coins'}))), {}),
        (dict(containers=dict(inventory=dict(slots={'0': None}))), {}),
        ({}, dict(hooks=[dict(event='replace', actions=[op('set', containers=dict(inventory=dict(slots={'0': 'shield'})))])])),
        (dict(nodes=dict(sword=dict(drop='coins'))), {}),
        (dict(nodes=dict(sword=dict(drop='shield'))), {}),
        (dict(nodes=dict(sword=dict(index=40))), {}),
    ]
    for (before, extra), stop in itertools.product(scripts, [False, True]):
        ops = [op('drop', node='sword')]
        if before:
            ops.append(op('set', **before))
        ops.append(op('script', n=0))
        out.append((case(ops, values=dict(usedOption='drop', ticks=2147483647), answers=dict(stopExecuting=stop), **extra), None))
    # Dropping coins, two drops queued in one tick, the dupe alert text.
    out.append((case([op('drop', node='coins'), op('script', n=0)], values=dict(usedOption='drop')), None))
    out.append((case([op('drop', node='sword'), op('drop', node='shield'), op('script', n=1), op('script', n=0)], values=dict(usedOption='drop')), None))
    out.append((case([op('drop', node='sword'), op('script', n=0)], values=dict(usedOption='drop'), nodes=dict(p=dict(name=None)),
                     hooks=[dict(event='get', actions=[op('set', containers=dict(inventory=dict(slots={'0': 'sword'})))])],
                     answers={}), None))
    # The dialogue action.
    for button, removed in itertools.product([1, 2, 3, 4], [False, True]):
        out.append((case([op('drop', node='sword'), op('dialogue', n=0, button=button)], values=dict(usedOption='destroy'), answers=dict(removeItem=removed)), None))
    # Registration; a non-item node answers false.
    out.append((case([op('drop_define'), op('invoke', n=0, node='goblin'), op('invoke', n=0, node='sword'), op('script', n=0)], values=dict(usedOption='drop')), None))
    out.append((case([op('drop_define'), op('invoke', n=0, node='loot')], values=dict(usedOption='destroy'), itemDefs={'526': dict(handlers={})}), None))
    out.append((case([op('drop', node='sword'), op('script', n=0)], values=dict(usedOption='drop')), 1))
    out.append((case([op('drop', node='sword')], values=dict(usedOption='drop')), 0))
    out.append((case([op('drop', node='sword'), op('dialogue', n=0, button=3)], values=dict(usedOption='destroy')), 1))
    return out


def cases():
    for family in [item_actions, npc_actions, player_actions, use_withs, components, slot_switches, examines, equips, drops]:
        for entry in family():
            if len(entry) == 1:
                entry = (entry[0], None)
            yield entry


def groups(cases):
    counted = {}
    for case in cases:
        first = case['ops'][0]['op'] if case['ops'][0]['op'] not in ('set', 'use_register', 'equip_define', 'drop_define') else case['ops'][-2]['op'] if len(case['ops']) > 1 else case['ops'][0]['op']
        counted[first] = counted.get(first, 0) + 1
    return dict(sorted(counted.items()))
