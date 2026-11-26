"""The activity and cutscene corpus.

Each case is (case, sweep): `sweep` is the index of an operation whose every recorded call is made to fail once in
turn by generate.py's second pass, or None. Templates are regions of fixtures/dynamic-regions/cache.json:
Lumbridge (12850), the Fight Caves (9551), Pest Control (10536), Clan Wars (13114/13115).
"""

LUMBRIDGE = 12850
CAVES = 9551
PEST = 10536
HOME = [3222, 3218, 0]
WORLD = {'settings': {'name': '2009scape', 'devMode': True}, 'home': HOME, 'logCutscene': True, 'serverName': '2009scape'}
PLAYER = {'name': 'player', 'location': [3222, 3218, 0]}

NEEDED_EVENTS = [
    'newInstance', 'contentConfigure', 'contentRegister', 'contentStart', 'open', 'fade', 'setup', 'runStage', 'submit', 'write',
    'removeTabs', 'restoreTabs', 'openDefaultTabs', 'closeInterface', 'openOverlay', 'closeOverlay', 'teleportLocation', 'spawnLocation',
    'safeZone', 'setLocation', 'setAttribute', 'getAttribute', 'removeAttribute', 'lock', 'unlock', 'walkingReset', 'lockMovement',
    'unlockMovement', 'sessionActive', 'packetMessage', 'hook', 'unhook', 'logoutListener', 'removeLogoutListener', 'addAction',
    'movementPulse', 'npcNew', 'npcRespawn', 'npcAggressive', 'npcWalks', 'npcInit', 'faceLocation', 'npcClear', 'antiMacroPause',
    'antiMacroUnpause', 'camera', 'cameraPanTo', 'cameraRotateTo', 'cameraRotateBy', 'cameraShake', 'cameraReset', 'log', 'stackTrace',
    'sendNPCDialogue', 'sendNPCDialogueLines', 'closeDialogue', 'sendDialogue', 'sendDialogueLines', 'sendPlayerDialogue', 'zoneUpdate',
]
NEEDED_ERRORS = ['Failure', 'Null', 'ClassCast', 'Uninitialized']


def case(name, ops, world=None, player=None, answers=None, group='activities'):
    c = {'name': name, 'group': group, 'world': world or WORLD, 'player': player or PLAYER, 'ops': ops}
    if answers:
        c['answers'] = answers
    return c


def activity(name, instanced=False, multicombat=False, safe=False, restrictions=(), spawn=None, configure=()):
    return {'kind': 'activity', 'name': name, 'instanced': instanced, 'multicombat': multicombat, 'safe': safe,
            'restrictions': list(restrictions), 'spawn': spawn, 'configure': list(configure)}


def cutscene_plugin(name, fade=None, spawn=None, configure=(), **extra):
    c = {'kind': 'cutscene', 'name': name, 'spawn': spawn, 'configure': list(configure)}
    if fade is not None:
        c['fade'] = fade
    c.update(extra)
    return c


CREATE = [{'do': 'create', 'region': LUMBRIDGE}, {'do': 'setRegionBase'}]


def activity_cases():
    yield case('a plain activity registered over borders', [
        {'op': 'register', 'config': activity('plain', configure=[{'do': 'register', 'borders': [3200, 3200, 3263, 3263]}])},
        {'op': 'getActivity', 'name': 'plain'},
        {'op': 'getActivity', 'name': 'missing'},
        {'op': 'start', 'name': 'plain', 'login': False, 'args': [1, 'two']},
        {'op': 'enter', 'plugin': 'activity#1'},
        {'op': 'leave', 'plugin': 'activity#1', 'logout': False},
        {'op': 'leave', 'plugin': 'activity#1', 'logout': True},
        {'op': 'fireEvent', 'plugin': 'activity#1', 'id': 'anything'},
    ]), 3
    yield case('a safe multicombat activity over two regions', [
        {'op': 'register', 'config': activity('arena', multicombat=True, safe=True, restrictions=['CANNON', 'FOLLOWERS'], spawn=[3210, 3215, 0],
                                              configure=[{'do': 'register', 'borders': [3190, 3200, 3263, 3263]}])},
        {'op': 'enter', 'plugin': 'activity#1'},
        {'op': 'leave', 'plugin': 'activity#1', 'logout': True},
    ]), 0
    yield case('an unknown activity in development mode', [
        {'op': 'start', 'name': 'nothing', 'login': True},
    ]), 0
    yield case('an unknown activity outside development mode', [
        {'op': 'start', 'name': 'nothing', 'login': True},
    ], world=dict(WORLD, settings={'name': '2009scape', 'devMode': False}))
    yield case('an unknown activity without settings', [
        {'op': 'start', 'name': 'nothing', 'login': False},
    ], world=dict(WORLD, settings=None)), None
    yield case('an instanced activity copies its region', [
        {'op': 'register', 'config': activity('instance', instanced=True, spawn=[3200, 3200, 0], configure=CREATE)},
        {'op': 'start', 'name': 'instance', 'login': False, 'args': ['a']},
        {'op': 'enter', 'plugin': 'activity#2'},
        {'op': 'leave', 'plugin': 'activity#2', 'logout': True},
        {'op': 'start', 'name': 'instance', 'login': True},
    ]), 1
    yield case('an instanced activity without a spawn location', [
        {'op': 'register', 'config': activity('nospawn', instanced=True, configure=CREATE)},
        {'op': 'start', 'name': 'nospawn', 'login': False},
        {'op': 'enter', 'plugin': 'activity#2'},
        {'op': 'leave', 'plugin': 'activity#2', 'logout': True},
    ]), 3
    yield case('an instanced multicombat activity', [
        {'op': 'register', 'config': activity('multi', instanced=True, multicombat=True, safe=True, configure=[{'do': 'create', 'region': CAVES}, {'do': 'setRegionBase'}])},
        {'op': 'start', 'name': 'multi', 'login': False},
        {'op': 'leave', 'plugin': 'activity#2', 'logout': False},
    ]), 1
    yield case('a region base without a region', [
        {'op': 'register', 'config': activity('baseless', instanced=True, configure=[{'do': 'setRegionBase'}])},
        {'op': 'start', 'name': 'baseless', 'login': False},
    ]), None
    yield case('a region base over an area of instances', [
        {'op': 'register', 'config': activity('area', instanced=True, multicombat=True,
                                              configure=[{'do': 'setRegionBaseArea', 'borders': [3264, 3712, 3328, 3840]}])},
        {'op': 'start', 'name': 'area', 'login': False},
    ]), 1
    yield case('a region base over an area of instances, single combat', [
        {'op': 'register', 'config': activity('area', instanced=True, configure=[{'do': 'setRegionBaseArea', 'borders': [3264, 3712, 3328, 3840]}])},
        {'op': 'start', 'name': 'area', 'login': False},
    ]), None
    yield case('an instance whose content fails to configure', [
        {'op': 'register', 'config': activity('broken', instanced=True, configure=[{'do': 'create', 'region': PEST}, {'do': 'fail'}])},
        {'op': 'start', 'name': 'broken', 'login': False},
    ]), None
    yield case('a failing instance outside development mode', [
        {'op': 'register', 'config': activity('broken', instanced=True, configure=[{'do': 'fail'}])},
        {'op': 'start', 'name': 'broken', 'login': False},
    ], world=dict(WORLD, settings={'name': '2009scape', 'devMode': False})), 1
    yield case('a failing instance without settings', [
        {'op': 'register', 'config': activity('broken', instanced=True, configure=[{'do': 'fail'}])},
        {'op': 'start', 'name': 'broken', 'login': False},
    ], world=dict(WORLD, settings=None)), None
    yield case('a non-instanced activity fails to configure on registration', [
        {'op': 'register', 'config': activity('early', configure=[{'do': 'emit', 'value': 1}, {'do': 'fail'}])},
        {'op': 'getActivity', 'name': 'early'},
    ]), None
    yield case('re-registering a name replaces the activity', [
        {'op': 'register', 'config': activity('twice')},
        {'op': 'register', 'config': activity('twice', safe=True)},
        {'op': 'getActivity', 'name': 'twice'},
        {'op': 'start', 'name': 'twice', 'login': False},
    ]), 1
    yield case('NPCs enter and leave an activity', [
        {'op': 'register', 'config': activity('npcs', safe=True, spawn=[3201, 3201, 0])},
        {'op': 'npc', 'name': 'guard', 'id': 9, 'location': [3223, 3219, 0]},
        {'op': 'enter', 'plugin': 'activity#1', 'entity': 'guard'},
        {'op': 'leave', 'plugin': 'activity#1', 'entity': 'guard', 'logout': True},
    ]), 2
    yield case('an activity without a home location', [
        {'op': 'register', 'config': activity('homeless', instanced=True, safe=True, spawn=[3200, 3200, 0])},
        {'op': 'start', 'name': 'homeless', 'login': False},
        {'op': 'enter', 'plugin': 'activity#2'},
        {'op': 'leave', 'plugin': 'activity#2', 'logout': True},
    ], world=dict(WORLD, home=None)), None


def plugin_cases():
    g = 'cutscene plugins'
    yield case('a cutscene plugin starts without fading', [
        {'op': 'register', 'config': cutscene_plugin('scene', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'scene', 'login': False},
        {'op': 'enter', 'plugin': 'cutscenePlugin#2'},
        {'op': 'stop', 'plugin': 'cutscenePlugin#2', 'fade': False},
    ], group=g), 1
    yield case('a cutscene plugin fades in', [
        {'op': 'register', 'config': cutscene_plugin('fading', configure=CREATE)},
        {'op': 'start', 'name': 'fading', 'login': False},
        {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1},
        {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 2},
    ], group=g), 1
    for at in range(2, 9):
        yield case(f'a fade-in failing at tick {at}', [
            {'op': 'register', 'config': cutscene_plugin('fading', configure=CREATE)},
            {'op': 'start', 'name': 'fading', 'login': False},
            {'op': 'tick', 'count': at - 1},
            {'op': 'tick', 'count': 1},
            {'op': 'tick', 'count': 1},
        ], group=g), 3
    yield case('a cutscene plugin fades out', [
        {'op': 'register', 'config': cutscene_plugin('out', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'out', 'login': False},
        {'op': 'stop', 'plugin': 'cutscenePlugin#2', 'fade': True},
        {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1},
        {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 2},
    ], group=g), 2
    for at in range(4, 7):
        yield case(f'a fade-out failing at tick {at}', [
            {'op': 'register', 'config': cutscene_plugin('out', fade=False, configure=CREATE)},
            {'op': 'start', 'name': 'out', 'login': False},
            {'op': 'unpause', 'plugin': 'cutscenePlugin#2'},
            {'op': 'tick', 'count': at - 1},
            {'op': 'tick', 'count': 1},
            {'op': 'tick', 'count': 1},
        ], group=g), 4
    yield case('a fade-out with an inactive session', [
        {'op': 'register', 'config': cutscene_plugin('quiet', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'quiet', 'login': False},
        {'op': 'stop', 'plugin': 'cutscenePlugin#2', 'fade': True},
        {'op': 'tick', 'count': 6},
    ], answers={'sessionActive': [False, True]}, group=g), None
    yield case('a fade-out without a real end attribute', [
        {'op': 'register', 'config': cutscene_plugin('noend', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'noend', 'login': False},
        {'op': 'attr', 'key': 'real-end', 'value': None},
        {'op': 'stop', 'plugin': 'cutscenePlugin#2', 'fade': True},
        {'op': 'tick', 'count': 5},
        {'op': 'attr', 'key': 'cutscene:original-loc', 'value': None},
        {'op': 'stop', 'plugin': 'cutscenePlugin#2', 'fade': True},
        {'op': 'tick', 'count': 6},
    ], group=g), None
    yield case('a fade-out with a mistyped real end attribute', [
        {'op': 'register', 'config': cutscene_plugin('typed', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'typed', 'login': False},
        {'op': 'attr', 'key': 'real-end', 'value': 7},
        {'op': 'stop', 'plugin': 'cutscenePlugin#2', 'fade': True},
        {'op': 'tick', 'count': 5},
    ], group=g), 4
    yield case('a cutscene plugin left on logout', [
        {'op': 'register', 'config': cutscene_plugin('logout', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'logout', 'login': False},
        {'op': 'npc', 'name': 'extra', 'id': 1, 'location': [64, 0, 0]},
        {'op': 'leave', 'plugin': 'cutscenePlugin#2', 'logout': True},
    ], group=g), 3
    yield case('a cutscene plugin left without logging out', [
        {'op': 'register', 'config': cutscene_plugin('pause', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'pause', 'login': False},
        {'op': 'leave', 'plugin': 'cutscenePlugin#2', 'logout': False},
        {'op': 'tick', 'count': 6},
    ], group=g), 2
    yield case('a cutscene plugin left before it started', [
        {'op': 'register', 'config': cutscene_plugin('idle', fade=False)},
        {'op': 'leave', 'plugin': 'cutscenePlugin#1', 'logout': True},
        {'op': 'end', 'plugin': 'cutscenePlugin#1'},
    ], group=g), 1
    yield case('a cutscene plugin ends with NPCs in its instance', [
        {'op': 'register', 'config': cutscene_plugin('crowd', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'crowd', 'login': False},
        {'op': 'npc', 'name': 'one', 'id': 1, 'location': [8, 8, 0]},
        {'op': 'npc', 'name': 'two', 'id': 2, 'location': [3, 3, 0]},
        {'op': 'npc', 'name': 'three', 'id': 3, 'location': [1, 1, 1]},
        {'op': 'npc', 'name': 'four', 'id': 4, 'location': [9, 1, 0]},
        {'op': 'end', 'plugin': 'cutscenePlugin#2'},
    ], group=g), 6
    yield case('a cutscene plugin without a region', [
        {'op': 'register', 'config': cutscene_plugin('plain', fade=False)},
        {'op': 'start', 'name': 'plain', 'login': True},
        {'op': 'end', 'plugin': 'cutscenePlugin#2'},
    ], group=g), 1
    yield case('a cutscene plugin with its own map state, tabs and start', [
        {'op': 'register', 'config': cutscene_plugin('custom', fade=False, configure=CREATE, mapState=0, tabs=[3, 7], startLocation=[3100, 3100, 1])},
        {'op': 'start', 'name': 'custom', 'login': False},
        {'op': 'stop', 'plugin': 'cutscenePlugin#2', 'fade': True},
        {'op': 'tick', 'count': 6},
    ], group=g), 1
    yield case('an end pulse of its own', [
        {'op': 'register', 'config': cutscene_plugin('own', fade=False, configure=CREATE)},
        {'op': 'start', 'name': 'own', 'login': False},
        {'op': 'submitEndPulse', 'plugin': 'cutscenePlugin#2'},
        {'op': 'tick', 'count': 2},
        {'op': 'active', 'value': False},
        {'op': 'tick', 'count': 1},
        {'op': 'submitStartPulse', 'plugin': 'cutscenePlugin#2'},
        {'op': 'tick', 'count': 2},
    ], group=g), None
    yield case('a start pulse submitted twice', [
        {'op': 'register', 'config': cutscene_plugin('twice', configure=CREATE)},
        {'op': 'start', 'name': 'twice', 'login': False},
        {'op': 'submitStartPulse', 'plugin': 'cutscenePlugin#2'},
        {'op': 'tick', 'count': 7},
    ], group=g), None
    yield case('a cutscene plugin started directly', [
        {'op': 'register', 'config': cutscene_plugin('direct', fade=False, configure=CREATE)},
        {'op': 'pluginStart', 'plugin': 'cutscenePlugin#1', 'login': False},
    ], group=g), 1


STAGES = {
    'setup': [{'do': 'loadRegion', 'region': LUMBRIDGE}],
    'stages': {
        '0': [{'do': 'addNPC', 'id': 2000, 'x': 20, 'y': 21, 'direction': 'NORTH'},
              {'do': 'addNPC', 'id': 17, 'x': 22, 'y': 21, 'direction': 'SOUTH_WEST', 'plane': 1},
              {'do': 'addNPC', 'id': 2000, 'x': 24, 'y': 20, 'direction': 'EAST'},
              {'do': 'addNPC', 'id': 33, 'x': 25, 'y': 25, 'direction': 'WEST'},
              {'do': 'teleport', 'x': 21, 'y': 19},
              {'do': 'teleport', 'npc': 17, 'x': 23, 'y': 22, 'plane': 2},
              {'do': 'moveCamera', 'x': 20, 'y': 20},
              {'do': 'rotateCamera', 'x': 25, 'y': 25, 'height': 400, 'speed': 90},
              {'do': 'npcDialogue', 'id': 2000, 'expression': 'HAPPY', 'message': 'Welcome to the cutscene.'}],
        '1': [{'do': 'move', 'x': 30, 'y': 31},
              {'do': 'move', 'npc': 33, 'x': 26, 'y': 26},
              {'do': 'getNPC', 'id': 2000}, {'do': 'getNPC', 'id': 99}, {'do': 'getNPCs', 'id': 2000}, {'do': 'getNPCs', 'id': 5},
              {'do': 'rotateCameraBy', 'x': 1, 'y': -1},
              {'do': 'shakeCamera', 'type': 'PAN'},
              {'do': 'shakeCamera', 'type': 'TILT', 'jitter': 1, 'amplitude': 2, 'frequency': 3, 'speed': 4},
              {'do': 'npcDialogueLines', 'id': 17, 'expression': 'NEUTRAL', 'lines': ['One.', 'Two.']}],
        '2': [{'do': 'dialogue', 'message': 'A plain message.'}],
        '3': [{'do': 'dialogueLines', 'lines': ['First line.', 'Second line.', 'Third line.']}],
        '4': [{'do': 'playerDialogue', 'expression': 'ASKING', 'message': 'And now?'}],
        '5': [{'do': 'timedUpdate', 'ticks': 2}],
        '6': [{'do': 'timedUpdate', 'ticks': 1, 'stage': 9}],
        '9': [{'do': 'resetCamera'}, {'do': 'dialogueClose'}, {'do': 'setExit', 'location': [3233, 3233, 0]},
              {'do': 'npcDialogue', 'id': 33, 'expression': 'NEUTRAL', 'message': 'Goodbye.', 'hide': True,
               'onContinue': [{'do': 'end'}]}],
    },
}


def cutscene_cases():
    g = 'cutscenes'
    yield case('a cutscene through every stage', [
        {'op': 'cutscene', 'script': STAGES},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'continue'}, {'op': 'continue'}, {'op': 'continue'}, {'op': 'continue'}, {'op': 'continue'},
        {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1},
        {'op': 'continue'},
        {'op': 'tick', 'count': 1},
        {'op': 'continue'},
        {'op': 'tick', 'count': 7}, {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1}, {'op': 'tick', 'count': 1},
    ], group=g), 1
    for sweep in (2, 3, 4, 5, 6, 8, 11):
        yield case(f'a cutscene through every stage failing at operation {sweep}', [
            {'op': 'cutscene', 'script': STAGES},
            {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
            {'op': 'continue'}, {'op': 'continue'}, {'op': 'continue'}, {'op': 'continue'}, {'op': 'continue'},
            {'op': 'tick', 'count': 2},
            {'op': 'continue'},
            {'op': 'tick', 'count': 1},
            {'op': 'continue'},
            {'op': 'tick', 'count': 9},
        ], group=g), sweep
    yield case('a cutscene ending without a fade', [
        {'op': 'cutscene', 'script': {'stages': {'0': [{'do': 'setExit', 'location': [3100, 3100, 0]}, {'do': 'end', 'fade': False}]}}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1', 'hide': False},
        {'op': 'tick', 'count': 1},
        {'op': 'tick', 'count': 1},
    ], group=g), 3
    yield case('a cutscene ending without fading through endWithoutFade', [
        {'op': 'cutscene', 'script': {'stages': {'0': [{'do': 'endWithoutFade'}]}}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'tick', 'count': 1},
        {'op': 'tick', 'count': 1},
        {'op': 'tick', 'count': 1},
    ], group=g), 3
    yield case('end actions run and fail', [
        {'op': 'cutscene', 'script': {'stages': {'0': [{'do': 'endWithoutFade', 'endActions': [{'do': 'emit', 'value': 'after'}, {'do': 'fail'}]}]}}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'tick', 'count': 2},
        {'op': 'cutscene', 'script': {'stages': {'0': [{'do': 'end', 'fade': False, 'endActions': [{'do': 'emit', 'value': 'faded'}]}]}}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#2'},
        {'op': 'tick', 'count': 1},
        {'op': 'cutscene', 'script': {'stages': {'0': [{'do': 'end', 'endActions': [{'do': 'fail'}]}]}}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#3'},
        {'op': 'tick', 'count': 9},
    ], group=g), 2
    yield case('a cutscene logged out', [
        {'op': 'cutscene', 'script': {'setup': [{'do': 'setExit', 'location': [3000, 3000, 0]}]}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'logout', 'key': 'cutscene'},
        {'op': 'tick', 'count': 9},
        {'op': 'logout', 'key': 'cutscene'},
    ], group=g), 2
    yield case('a cutscene player dies', [
        {'op': 'cutscene', 'script': {}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'death'},
        {'op': 'tick', 'count': 9},
        {'op': 'death'},
    ], group=g), 2
    yield case('a death hook without a cutscene unhooks itself', [
        {'op': 'cutscene', 'script': {}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'attr', 'key': 'cutscene', 'value': None},
        {'op': 'death'},
        {'op': 'death'},
    ], group=g), 3
    yield case('a mistyped cutscene attribute', [
        {'op': 'cutscene', 'script': {}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'attr', 'key': 'cutscene', 'value': 5},
        {'op': 'death'},
        {'op': 'logout', 'key': 'cutscene'},
    ], group=g), None
    yield case('a cutscene resumes from a stored stage', [
        {'op': 'attr', 'key': 'cutscene:stage', 'value': 3},
        {'op': 'cutscene', 'script': STAGES},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'continue'},
    ], group=g), 2
    yield case('a mistyped stage attribute', [
        {'op': 'attr', 'key': 'cutscene:stage', 'value': 'two'},
        {'op': 'cutscene', 'script': {}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
    ], group=g), None
    yield case('a cutscene used before it starts', [
        {'op': 'cutscene', 'script': {}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'teleport', 'x': 1, 'y': 2}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'moveCamera', 'x': 1, 'y': 2}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 1, 'y': 2}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'addNPC', 'id': 5, 'x': 1, 'y': 2, 'direction': 'NORTH'}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'incrementStage'}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'shakeCamera', 'type': 'TRUCK'}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'clearNPCs'}},
    ], group=g), None
    yield case('objects of the cutscene region', [
        {'op': 'cutscene', 'script': {'setup': [{'do': 'loadRegion', 'region': LUMBRIDGE}]}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 18, 'y': 22}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 0, 'y': 0}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 13, 'y': 22, 'plane': 1}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 26, 'y': 18}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 16, 'y': 31}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'loadRegion', 'region': CAVES}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 20, 'y': 30}},
    ], group=g), 2
    yield case('objects of the player\'s own region', [
        {'op': 'cutscene', 'script': {}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1', 'hide': True},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 18, 'y': 22}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'getObject', 'x': 22, 'y': 18}},
    ], group=g), None
    yield case('a cutscene loading regions twice clears its NPCs', [
        {'op': 'cutscene', 'script': {'setup': [{'do': 'loadRegion', 'region': PEST},
                                                {'do': 'addNPC', 'id': 70, 'x': 1, 'y': 1, 'direction': 'NORTH'},
                                                {'do': 'addNPC', 'id': 6, 'x': 2, 'y': 1, 'direction': 'NORTH_EAST'},
                                                {'do': 'addNPC', 'id': 70, 'x': 3, 'y': 1, 'direction': 'SOUTH'},
                                                {'do': 'loadRegion', 'region': CAVES},
                                                {'do': 'addNPC', 'id': 4, 'x': 1, 'y': 9, 'direction': 'SOUTH_EAST'}]}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'clearNPCs'}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'clearNPCs'}},
    ], group=g), 1
    many = [{'do': 'addNPC', 'id': i * 37 % 101, 'x': i % 30, 'y': i % 7, 'direction': 'NORTH'} for i in range(1, 30)]
    yield case('a cutscene with many NPC ids', [
        {'op': 'cutscene', 'script': {'setup': many}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'clearNPCs'}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'addNPC', 'id': 3, 'x': 1, 'y': 1, 'direction': 'NORTH'}},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'clearNPCs'}},
    ], group=g), None
    yield case('a cutscene without cutscene logs', [
        {'op': 'cutscene', 'script': STAGES},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'continue'},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'end'}},
        {'op': 'tick', 'count': 9},
    ], world=dict(WORLD, logCutscene=False), group=g), 1
    yield case('custom continue actions and stage updates after the end', [
        {'op': 'cutscene', 'script': {'stages': {
            '0': [{'do': 'dialogue', 'message': 'Jump.', 'onContinue': [{'do': 'updateStage', 'stage': 4}]}],
            '4': [{'do': 'playerDialogue', 'expression': 'NEUTRAL', 'message': 'Stop.', 'onContinue': [{'do': 'emit', 'value': 'kept'}]},
                  {'do': 'dialogueLines', 'lines': ['x'], 'onContinue': [{'do': 'end', 'fade': False}, {'do': 'incrementStage'}]}],
        }}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
        {'op': 'continue'},
        {'op': 'continue'},
        {'op': 'cutsceneCall', 'cutscene': 'cutscene#1', 'action': {'do': 'updateStage', 'stage': 0}},
        {'op': 'tick', 'count': 1},
    ], group=g), 3
    yield case('a cutscene in an instance of its own region', [
        {'op': 'place', 'location': [2400, 5100, 0]},
        {'op': 'cutscene', 'script': {'stages': {'0': [{'do': 'teleport', 'x': 10, 'y': 10}, {'do': 'move', 'x': 1, 'y': 2}]}}},
        {'op': 'startCutscene', 'cutscene': 'cutscene#1'},
    ], group=g), 2


def cases():
    for generator in (activity_cases, plugin_cases, cutscene_cases):
        for item in generator():
            if isinstance(item, tuple):
                yield item
            else:
                yield item, None


def groups(cases):
    found = {}
    for c in cases:
        found[c['group']] = found.get(c['group'], 0) + 1
    return dict(sorted(found.items()))
