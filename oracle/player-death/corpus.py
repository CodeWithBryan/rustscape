"""The designed corpus of the player death oracle (see generate.py).

Five groups: items kept on death (getContainers) over many inventories and
every mode combination; the player death sequence (DeathTask start/exec/stop,
commenceDeath, finalizeDeath) in safe, dangerous and wilderness-like zones;
graves of every type over whole lifetimes with looting checks at the time
boundaries, blessing, repair, reading, demolition, disconnect and relog;
the server-store save and parse of active graves; and the static item rules.
"""
import json
import random
import struct

INT_MIN, INT_MAX = -(1 << 31), (1 << 31) - 1

NEEDED = ['skulled', 'prayer', 'ironmanMode', 'graveNew', 'transform', 'init', 'clear', 'flagged', 'packet', 'hintIcon', 'clearHintIcon',
          'sendMessage', 'playerMessage', 'dispatchMessage', 'repositoryNews', 'permadeath', 'deathItem', 'equipmentEvent', 'unequipListeners',
          'monitor', 'retribution', 'openComponent', 'varbit', 'interfaceText', 'audio', 'animate', 'setPrayerPoints', 'uidMap', 'archive',
          'wildernessZone', 'zoneRestricted', 'tick', 'npcLocation', 'teleport', 'visualize', 'setDisableDeathScreen', 'closeInterface', 'pluginRemove']


def bits(value):
    return str(struct.unpack('<q', struct.pack('<d', value))[0])


def a(op, **kw):
    return dict(op=op, **kw)


def plugin(id, death):
    return dict(id=id, deathItem=death)


# Item definitions. Values are chosen for alchemy ties and their order.
ITEMS = {
    '995': dict(name='Coins', value=1, stackable=True, handlers=dict(tradeable=True)),
    '554': dict(name='Fire rune', value=4, stackable=True, handlers=dict(tradeable=True)),
    '560': dict(name='Death rune', value=180, stackable=True, handlers=dict(tradeable=True)),
    '526': dict(name='Bones', value=1, handlers=dict(tradeable=True)),
    '1511': dict(name='Logs', value=4, handlers=dict(tradeable=True)),
    '1512': dict(name='Logs', value=1, unnoted=False, noteId=1511, handlers=dict(tradeable=True)),
    '1163': dict(name='Rune full helm', value=35200, handlers=dict(tradeable=True)),
    '1127': dict(name='Rune platebody', value=65000, handlers=dict(tradeable=True)),
    '1333': dict(name='Rune scimitar', value=25600, handlers=dict(tradeable=True)),
    '1319': dict(name='Rune 2h sword', value=25600, handlers=dict(tradeable=True)),
    '1215': dict(name='Dragon dagger', value=30000, handlers=dict(tradeable=True)),
    '1305': dict(name='Dragon longsword', value=100000, handlers=dict(tradeable=True)),
    '4151': dict(name='Abyssal whip', value=120001, handlers=dict(tradeable=True)),
    '4152': dict(name='Abyssal whip', value=1, unnoted=False, noteId=4151, handlers=dict(tradeable=True)),
    '6570': dict(name='Fire cape', value=50000, handlers=dict(tradeable=False)),
    '7462': dict(name='Barrows gloves', value=10, handlers=dict(high_alchemy=60000)),
    '1704': dict(name='Amulet of glory', value=17625, handlers=dict(tradeable=True, equipment=31)),
    '1': dict(name='Toolkit', value=0, handlers=dict(tradeable=True)),
    '2': dict(name='Cannonball', value=5, stackable=True, handlers=dict(tradeable=True)),
    '3': dict(name='Deflated thing', value=-5, handlers=dict(tradeable=True)),
    '4': dict(name='Heavy thing', value=INT_MAX, handlers=dict(tradeable=True)),
    '2677': dict(name='Clue scroll', value=1, options=[None, None, None, None, 'Destroy'], handlers=dict(tradeable=True)),
    '4251': dict(name='Ectophial', value=1, handlers=dict(tradeable=False)),
    '5509': dict(name='Small pouch', value=1, handlers=dict(tradeable=False)),
    '5515': dict(name='Giant pouch', value=1, handlers=dict(tradeable=False)),
    '5516': dict(name='Giant pouch', value=1, handlers=dict(tradeable=False)),
    '9976': dict(name='Chinchompa', value=1, stackable=True, handlers=dict(tradeable=True)),
    '10033': dict(name='Chinchompa', value=1, stackable=True, handlers=dict(tradeable=True)),
    '10034': dict(name='Red chinchompa', value=1, stackable=True, handlers=dict(tradeable=True)),
    '11238': dict(name='Baby impling jar', value=1, options=[None, None, None, None, 'Destroy'], handlers=dict(tradeable=True)),
    '11239': dict(name='Baby impling jar', value=1, unnoted=False, noteId=11238, handlers=dict(tradeable=True)),
    '11256': dict(name='Dragon impling jar', value=1, options=[None, None, None, None, 'Destroy'], handlers=dict(tradeable=True)),
    '4716': dict(name="Dharok's helm", value=100000, handlers=dict(tradeable=True)),
    '4880': dict(name="Dharok's helm 100", value=100000, handlers=dict(tradeable=False)),
    '4883': dict(name="Dharok's helm 25", value=100000, handlers=dict(tradeable=False)),
    '4884': dict(name="Dharok's helm 0", value=100000, handlers=dict(tradeable=True)),
    '4718': dict(name="Dharok's greataxe", value=100000, handlers=dict(tradeable=True)),
    '4890': dict(name="Dharok's axe 0", value=100000, handlers=dict(tradeable=True)),
    '4675': dict(name='Ancient staff', value=80000, handlers=dict(tradeable=True, wrapper=plugin(7, 'same'))),
    '4676': dict(name='Changing staff', value=80000, handlers=dict(tradeable=True, wrapper=plugin(8, [1511, 3, 77]))),
    '4677': dict(name='Vanishing staff', value=80000, handlers=dict(tradeable=True, wrapper=plugin(9, None))),
    '14484': dict(name='Dragon claws', value=200000, handlers=dict(tradeable=True)),
    '7': dict(name=None, value=50, handlers=dict(tradeable=True)),
}

POOL = ['995', '554', '560', '1511', '1512', '1163', '1127', '1333', '1319', '1215', '1305', '4151', '4152', '6570', '7462', '1704', '1', '2', '3']
MODES = [None, 'NONE', 'STANDARD', 'HARDCORE', 'ULTIMATE']
HERE = [3200, 3200, 0]


def victim(**kw):
    base = dict(name='victim', username='Victim', uid=10, location=HERE, levels={'5': 70, '6': 40}, runEnergy=bits(73.5), totalLevel=500,
                mode='NONE', prayerPoints=bits(43.0))
    base.update(kw)
    return base


def killer(**kw):
    base = dict(name='killer', username='Killer', uid=20, location=[3201, 3200, 0], levels={'5': 80, '6': 60}, mode='NONE', prayerPoints=bits(70.0))
    base.update(kw)
    return base


def kept_cases(cases):
    rng = random.Random(530)

    def add(inventory, equipment=None, **flags):
        cases.append(dict(items=ITEMS, players=[victim(inventory=inventory, equipment=equipment or [], **flags)], actions=[a('containers', player='p0')]))

    sets = [
        [],
        [[1163, 1]],
        [[1163, 1], [1127, 1]],
        [[1163, 1], [1127, 1], [4151, 1]],
        [[1163, 1], [1127, 1], [4151, 1], [1305, 1]],
        [[1163, 1], [1127, 1], [4151, 1], [1305, 1], [1215, 1]],
        [[995, 1000000]],
        [[995, 5], [554, 3]],
        [[560, 1000], [1163, 1]],
        [[560, 2], [1163, 1], [1127, 1], [995, 100]],
        [[1333, 1], [1319, 1], [1333, 1], [1319, 1]],
        [[1319, 1], [1333, 1]],
        [[4152, 5], [4151, 1]],
        [[1512, 20], [1511, 1], [1511, 1]],
        [[6570, 1], [7462, 1], [1163, 1]],
        [[1, 1], [1, 1], [1, 1], [1, 1]],
        [[3, 1], [1, 1], [3, 1]],
        [[4, 1], [4, 1], [1163, 1]],
        [[2, 5000], [995, 100000], [554, 77]],
        [[4151, 1, 500], [4151, 1, 1000], [4151, 1, 0]],
        [[1704, 1, 4], [1704, 1, 3], [1704, 1, 2], [1704, 1, 1]],
    ]
    for items in sets:
        for skulled in [False, True]:
            for protect in [False, True]:
                for mode in MODES:
                    add(items, skulled=skulled, protect=protect, mode=mode)
    # Inventory and equipment together: duplicates merge (the later charge wins), equipment comes second.
    for equipment in [[[1163, 1]], [None, [4151, 1], None, [1127, 1]], [[1333, 1], [1319, 1]], [[4151, 1, 7]], [[995, 10]]]:
        for protect in [False, True]:
            add([[1163, 1], [4151, 1, 9], [995, 3], [1319, 1]], equipment, protect=protect)
            add([[1333, 1], [1319, 1]], equipment, protect=protect, skulled=True)
    # Full inventory and full equipment: displaced kept items cannot always go back.
    distinct = [[int(id), 1] for id in POOL if ITEMS[id].get('stackable') is not True and ITEMS[id].get('unnoted', True)]
    full_inventory = [distinct[i % len(distinct)] for i in range(28)]
    full_equipment = [distinct[(i * 5 + 3) % len(distinct)] for i in range(14)]
    many = [[int(id), 1] for id in ['1163', '1127', '1333', '1319', '1215', '1305', '4151', '6570', '7462', '1704', '1', '3', '4', '1511', '4716',
                                     '4718', '4884', '4890', '14484', '2677', '4251', '5509', '5515', '11238', '4675', '4676', '4677', '526']]
    many_equipment = [[int(id), 1] for id in ['4880', '4883', '7', '5516', '11256', '1704', '1305', '4151', '1163', '1127', '1333', '1319', '1215', '4']]
    for protect in [False, True]:
        for skulled in [False, True]:
            add(full_inventory, full_equipment, protect=protect, skulled=skulled)
            add(many, many_equipment, protect=protect, skulled=skulled)
    # Seeded random inventories and equipment over the pool.
    for _ in range(260):
        inventory = []
        for slot in range(28):
            if rng.random() < 0.45:
                id = rng.choice(POOL)
                stackable = ITEMS[id].get('stackable') or not ITEMS[id].get('unnoted', True)
                amount = rng.choice([1, 2, 3, 50, 10000]) if stackable else 1
                charge = rng.choice([1000, 1000, 1000, 1, 999])
                inventory.append([int(id), amount, charge])
            else:
                inventory.append(None)
        equipment = [[int(rng.choice(POOL)), 1, 1000] if rng.random() < 0.4 else None for _ in range(14)]
        add(inventory, equipment, skulled=rng.random() < 0.3, protect=rng.random() < 0.5, mode=rng.choice(MODES))


def death_cases(cases):
    inventory = [[1163, 1], [995, 5000], [6570, 1], [4151, 1], [2677, 1], [9976, 30], [11238, 1], [11239, 2], [4716, 1], [4880, 1, 900],
                 [4884, 1], [4675, 1], [4676, 1], [5509, 1], [4251, 1], [10034, 4], [1305, 1], [7462, 1]]
    equipment = [[1704, 1], None, [1127, 1], [4718, 1], None, None, None, [4883, 1, 12]]

    def add(actions, players, npcs=None, **kw):
        cases.append(dict(items=ITEMS, players=players, npcs=npcs or [dict(name='Goblin', id=1, location=[3199, 3200, 0])], actions=actions,
                          order=['graves', 'ground', 'npcs'], tick=kw.pop('tick', 1000), **kw))

    def victim_(**kw):
        return victim(inventory=inventory, equipment=equipment, **kw)

    finalize = a('finalize', player='p0', killer='p1')
    by_npc = a('finalize', player='p0', killer='n0')
    # Who killed: nobody, self, another player (active, inactive, same name), an NPC, an artificial victim.
    for k in [None, 'p0', 'p1', 'n0', 'p2']:
        for artificial in [False, True]:
            add([a('finalize', player='p0', killer=k)], [victim_(skulled=True, artificial=artificial), killer(), killer(name='victim', uid=30, username='Victim')])
    add([finalize], [victim_(skulled=True), killer(active=False)])
    add([finalize, a('commence', player='p0', killer='p1')], [victim_(playing=False), killer()])
    # commenceDeath: retribution.
    for retribution in [False, True]:
        for k in [None, 'p1', 'n0']:
            add([a('commence', player='p0', killer=k)], [victim_(retribution=retribution), killer()])
    # Safe zones end the death before any item moves.
    for change in [dict(zoneHandle=True), dict(safeZone=True), dict(zoneType=1), dict(zoneType=2), dict(rights='ADMINISTRATOR'),
                   dict(rights='PLAYER_MODERATOR'), dict(zoneType=1, safeZone=True), dict(deathScreenDisabled=True)]:
        add([finalize], [victim_(**change), killer()])
        add([by_npc], [victim_(**change)])
    # Ironman modes, hardcore permadeath, a missing mode.
    for mode in MODES:
        for permadeath in [False, True]:
            for male in [True, False]:
                attributes = {'permadeath': True} if permadeath else {}
                add([by_npc], [victim_(mode=mode, male=male, attributes=attributes)])
    # Why a grave is not made.
    for change in [dict(skulled=True), dict(wilderness=True), dict(inWilderness=True), dict(mode='HARDCORE'), dict(restrictGraves=True), {}]:
        for k in ['p1', 'n0', None]:
            add([a('finalize', player='p0', killer=k)], [victim_(**change), killer()])
    # Magic level halves the value an untradeable item gives a player killer.
    for magic in [1, 54, 55, 99]:
        add([finalize], [victim_(skulled=True), killer(levels={'6': magic})])
    # Kept items with protect item and skull, original location attribute.
    for protect in [False, True]:
        add([finalize], [victim_(skulled=True, protect=protect, attributes={'/save:original-loc': {'loc': [3300, 3300, 1]}}), killer()])
        add([by_npc], [victim_(protect=protect, attributes={'/save:original-loc': {'loc': [3300, 3300, 1]}})])
    # Grave types from the persisted attribute, including values out of range and of another type.
    for ordinal in list(range(13)) + [13, -1, 'x']:
        add([by_npc], [victim_(attributes={'/save:gravetype': ordinal})])
    # A plugin that gives no death item fails the death.
    add([by_npc], [victim(inventory=[[4677, 1]])])
    add([a('finalize', player='p0', killer='p1')], [victim(inventory=[[4677, 1]], skulled=True), killer()])
    # Nothing left for a grave: no grave is registered.
    add([by_npc], [victim(inventory=[[1163, 1], [4151, 1]])])
    add([by_npc], [victim(inventory=[[9976, 5], [4251, 1]])])
    add([by_npc], [victim(inventory=[])])
    # An item without a name.
    add([by_npc], [victim(inventory=[[1163, 1], [4151, 1], [1305, 1], [7, 1]])])
    add([by_npc], [victim(inventory=[[11238, 1], [7, 1], [1163, 1], [4151, 1], [1305, 1]], attributes={'/save:gravetype': 12})])
    # The whole DeathTask over a player: start, exec before and after the animation, stop.
    for anim in [[836, 4], [836, 0], [836, 31], [836, 30], [836, 1]]:
        for k in [None, 'p1', 'n0']:
            add([a('start', player='p0', killer=k), a('exec', player='p0', killer=k), a('set', tick=1003), a('exec', player='p0', killer=k),
                 a('set', tick=1004), a('exec', player='p0', killer=k), a('set', tick=1006), a('exec', player='p0', killer=k),
                 a('set', tick=1030), a('exec', player='p0', killer=k), a('stop', player='p0', killer=k)],
                [victim_(deathAnimation=anim, spawn=[3222, 3218, 0], retribution=k == 'p1'), killer()])
    add([a('start', player='p0', killer='p1'), a('stop', player='p0', killer='p1')], [victim_(safeZone=True, spawn=[3222, 3218, 0]), killer()])
    add([a('start', player='p0', killer='p1'), a('stop', player='p0', killer='p1')], [victim_(playing=False, spawn=[3222, 3218, 0]), killer()])
    # Second death while a grave is active: the old grave collapses.
    add([by_npc, a('run', ticks=5), by_npc], [victim_()])
    add([by_npc, a('run', ticks=5), a('finalize', player='p1', killer='n0'), a('run', ticks=3), by_npc], [victim_(), killer(inventory=[[1163, 1], [1127, 1], [1305, 1], [4151, 1], [1215, 1]])])


def grave_cases(cases):
    durations = [2, 2, 2, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5]
    inventory = [[1163, 1], [995, 5000], [4151, 1], [1127, 1], [1305, 1], [1215, 1], [6570, 1], [554, 40], [11238, 1], [4880, 1, 900]]

    def add(actions, players, **kw):
        cases.append(dict(items=ITEMS, players=players, npcs=[dict(name='Goblin', id=1, location=[3199, 3200, 0])], actions=actions,
                          order=kw.pop('order', ['graves', 'ground', 'npcs']), tick=kw.pop('tick', 1000), **kw))

    def looks(players, id=995, location=HERE):
        return [a('get', player=p, id=id, location=location) for p in players]

    # Every grave type over its whole life, looted by its owner and by another player at the boundaries.
    for ordinal, minutes in enumerate(durations):
        ticks = minutes * 100
        actions = [a('finalize', player='p0', killer='n0')] + looks(['p0', 'p1'])
        actions += [a('run', ticks=ticks - 160)] + looks(['p0', 'p1']) + [a('run', ticks=15), a('run', ticks=100)] + looks(['p0', 'p1', 'p2'])
        actions += [a('run', ticks=30)]
        for _ in range(16):
            actions += [a('tick')] + looks(['p0', 'p1'])
        actions += [a('run', ticks=3)] + looks(['p0'])
        add(actions, [victim(inventory=inventory, attributes={'/save:gravetype': ordinal}), killer(), killer(name='third', uid=30, username='Third')],
            tick=[0, 1000, 37, 123456][ordinal % 4])
    # The driver order changes which happens first: an item expiring or the grave collapsing.
    for order in [['ground', 'graves', 'npcs'], ['npcs', 'graves', 'ground'], ['graves']]:
        actions = [a('finalize', player='p0', killer='n0'), a('run', ticks=195)]
        for _ in range(8):
            actions += [a('tick')] + looks(['p0'])
        add(actions, [victim(inventory=inventory)], order=order)
    # Near the integer limit the decay tick wraps.
    add([a('finalize', player='p0', killer='n0')] + looks(['p0', 'p1']) + [a('tick')] + looks(['p0']) + [a('run', ticks=200)] + looks(['p0']),
        [victim(inventory=inventory), killer()], tick=INT_MAX - 150)
    # Disconnect and relog of the owner during the grave's life (p2 is the owner's new player object).
    owner_again = victim(inventory=[], online=False, username='Victim')
    add([a('finalize', player='p0', killer='n0'), a('run', ticks=2), a('logout', player='p0'), a('run', ticks=2)] + looks(['p0', 'p2'])
        + [a('login', player='p2'), a('run', ticks=2)] + looks(['p2', 'p0', 'p1'], id=1163)
        + [a('take', player='p2', id=1163, location=HERE), a('take', player='p1', id=995, location=HERE), a('run', ticks=60), a('logout', player='p2'),
           a('run', ticks=140)] + looks(['p2']), [victim(inventory=inventory), killer(), owner_again])
    add([a('finalize', player='p0', killer='n0'), a('logout', player='p0'), a('run', ticks=205)], [victim(inventory=inventory)])
    add([a('finalize', player='p0', killer='n0'), a('logout', player='p0'), a('collapse', grave='g0'), a('login', player='p0'), a('collapse', grave='g0')],
        [victim(inventory=inventory)])
    # Blessing: the refusals in order, then blessings of several sizes.
    for change, by in [({}, 'p1'), ({}, 'p0'), (dict(mode='STANDARD'), 'p1'), (dict(mode='ULTIMATE'), 'p1')]:
        add([a('finalize', player='p0', killer='n0'), a('bless', player=by, grave='g0'), a('bless', player=by, grave='g0')],
            [victim(inventory=inventory, **change), killer()])
    for level, points in [(69, 99.0), (70, 10.0), (70, 10.9), (70, 11.0), (70, 45.5), (99, 70.0), (99, 99.0), (120, 0.0), (70, -3.0)]:
        add([a('finalize', player='p0', killer='n0'), a('run', ticks=10), a('bless', player='p1', grave='g0'), a('read', player='p1', grave='g0'),
             a('run', ticks=10)], [victim(inventory=inventory), killer(levels={'5': level}, prayerPoints=bits(points))])
    add([a('finalize', player='p0', killer='n0'), a('logout', player='p0'), a('bless', player='p1', grave='g0')], [victim(inventory=inventory), killer()])
    # Repair.
    for level, points in [(1, 50.0), (2, 0.5), (2, 0.99), (2, 1.0), (2, 3.7), (40, 5.0), (40, 50.0)]:
        add([a('finalize', player='p0', killer='n0'), a('repair', player='p1', grave='g0'), a('repair', player='p1', grave='g0'),
             a('repair', player='p0', grave='g0')], [victim(inventory=inventory), killer(levels={'5': level}, prayerPoints=bits(points))])
    # Blessing and repair near the transform thresholds.
    for remaining in [10, 29, 30, 31, 89, 90, 91, 140]:
        add([a('finalize', player='p0', killer='n0'), a('setGrave', grave='g0', ticksRemaining=remaining), a('repair', player='p1', grave='g0'),
             a('addTime', grave='g0', ticks=-remaining), a('addTime', grave='g0', ticks=29), a('addTime', grave='g0', ticks=60), a('run', ticks=2)],
            [victim(inventory=inventory), killer(levels={'5': 50}, prayerPoints=bits(0.25 if remaining == 10 else 5.0))])
    # Reading every type and the formatted durations.
    for ordinal in range(13):
        add([a('finalize', player='p0', killer='n0'), a('read', player='p0', grave='g0'), a('read', player='p1', grave='g0'), a('run', ticks=133),
             a('read', player='p1', grave='g0')], [victim(inventory=inventory, attributes={'/save:gravetype': ordinal}), killer()])
    for remaining in [-101, -1, 0, 1, 2, 99, 100, 101, 102, 199, 200, 201, 4800, INT_MAX]:
        add([a('finalize', player='p0', killer='n0'), a('setGrave', grave='g0', ticksRemaining=remaining, ticksSinceDeath=remaining), a('read', player='p1', grave='g0')],
            [victim(inventory=inventory), killer()])
    add([a('read', player='p0', grave='n0'), a('bless', player='p0', grave='n0'), a('repair', player='p0', grave='n0'), a('demolish', player='p0', grave='n0')],
        [victim(inventory=inventory)])
    # Demolition: by someone else, by the owner, by an owner who has left.
    add([a('finalize', player='p0', killer='n0'), a('demolish', player='p1', grave='g0'), a('run', ticks=10), a('demolish', player='p0', grave='g0'),
         a('run', ticks=74)] + looks(['p0']) + [a('tick')] + looks(['p0']) + [a('run', ticks=5)], [victim(inventory=inventory), killer()])
    add([a('finalize', player='p0', killer='n0'), a('take', player='p0', id=995, location=HERE), a('demolish', player='p0', grave='g0'), a('run', ticks=80)],
        [victim(inventory=inventory)])
    add([a('finalize', player='p0', killer='n0'), a('logout', player='p0'), a('demolish', player='p0', grave='g0'), a('run', ticks=210)],
        [victim(inventory=inventory)])
    # Several graves: map order, a grave at -1 stops the controller's tick for those after it.
    owners = [victim(name='o' + str(uid), username='O' + str(uid), uid=uid, inventory=[[1163, 1], [4151, 1], [1305, 1], [1215, 1]]) for uid in [3, 19, 35, 16, 1, 1 << 20]]
    deaths = [a('finalize', player='p' + str(i), killer='n0') for i in range(len(owners))]
    add(deaths + [a('run', ticks=3), a('setGrave', grave='g2', ticksRemaining=-1), a('run', ticks=3), a('setGrave', grave='g2', ticksRemaining=0), a('run', ticks=2),
                  a('save')], owners)
    add(deaths + [a('run', ticks=150), a('save'), a('run', ticks=60), a('save')], owners)
    # Ground drops without a grave: private to the killer for 100 ticks, public until 200, untradeables stay private.
    for k in ['p1', 'n0']:
        actions = [a('finalize', player='p0', killer=k)]
        for at in [1000, 1099, 1100, 1101, 1199, 1200, 1201]:
            actions += [a('set', tick=at - 1), a('tick')] + looks(['p0', 'p1', 'p2']) + looks(['p0', 'p1'], id=6570)
        add(actions, [victim(inventory=inventory, skulled=True), killer(), killer(name='third', uid=30, username='Third')])
    add([a('finalize', player='p0', killer='p1'), a('logout', player='p1'), a('run', ticks=201)], [victim(inventory=inventory, skulled=True), killer()])
    add([a('finalize', player='p0', killer='p1'), a('setPlayer', player='p1', key='location', value=[3300, 3300, 0]), a('run', ticks=201)],
        [victim(inventory=inventory, skulled=True), killer()])


def store_cases(cases):
    def add(actions, players=None, **kw):
        cases.append(dict(items=ITEMS, players=players or [], actions=actions, order=['graves', 'ground', 'npcs'], tick=kw.pop('tick', 500), **kw))

    def grave(type, ticks, location, username, items, since=None):
        g = {'type': type, 'ticksRemaining': ticks, 'location': location, 'username': username, 'items': items}
        if since is not None:
            g['ticksSinceDeath'] = since
        return g

    items = [{'id': 1163, 'amount': 1, 'charge': 1000}, {'id': 995, 'amount': 300, 'charge': 1000}, {'id': 6570, 'amount': 1, 'charge': 7}]
    archive = {'10': grave(3, 250, '[3200, 3200, 0]', 'Victim', items, 40), '3': grave(0, 50, '[1, 2, 3]', 'O3', items[:1]),
               '100': grave(12, 101, '[3222, 3218, 1]', 'O100', [], 7), '19': grave(5, 99, '[3200, 3201, 0]', 'O19', items[1:], 0)}
    text = json.dumps(archive, separators=(',', ':'))
    owner = victim(inventory=[])
    add([a('parse', archive=text), a('save')] + [a('get', player='p0', id=1163, location=HERE)] + [a('run', ticks=60), a('save'), a('run', ticks=200), a('save')], [owner])
    for bad in ['{"5":{"type":13,"ticksRemaining":5,"location":"[1, 1, 0]","username":"x","items":[]}}',
                '{"5":{"type":"2","ticksRemaining":"5","location":"[1, 1, 0]","username":"x","items":[{"id":"995","amount":"3","charge":"1000"}]}}',
                '{"5":{"type":2,"ticksRemaining":5,"location":"[1, 1]","username":"x","items":[]}}',
                '{"x":{"type":2,"ticksRemaining":5,"location":"[1, 1, 0]","username":"x","items":[]}}',
                '{"5":{"type":2,"ticksRemaining":5.5,"location":"[1, 1, 0]","username":"x","items":[]}}',
                '{"5":{"type":2,"ticksRemaining":5,"location":"[1, 1, 0]","items":[]}}',
                '{"5":{"type":2,"ticksRemaining":5,"location":"[1, 1, 0]","username":"a\\"b/c","items":[]},"6":{"type":1,"ticksRemaining":-1,"location":"[1, 1, 0]","username":"y","items":[]}}',
                '{}']:
        add([a('parse', archive=bad), a('save'), a('run', ticks=2), a('save')])
    # Two archive keys that share a String hash bucket.
    add([a('parse', archive=json.dumps({'Aa': grave(1, 9, '[0, 0, 0]', 'a', []), 'BB': grave(2, 9, '[0, 0, 0]', 'b', [])}))])


def static_cases(cases):
    ids = [4250, 4251, 4252, 5508, 5509, 5510, 5511, 5512, 5513, 5514, 5515, 5516, 9975, 9976, 9977, 10032, 10033, 10034, 11237, 11238, 11239, 11256,
           11257, 11258, 2677, 1163, 0, -1, 65535, 65536]
    actions = []
    for id in ids:
        actions += [a('crumble', id=id), a('release', id=id)]
    for item in [[4675, 1], [4676, 2], [1163, 1], [4677, 1], [1163, 0]]:
        actions.append(a('checkTransform', item=item))
    actions.append(a('types'))
    cases.append(dict(items=ITEMS, actions=actions))
    barrows = list(range(4706, 4762)) + list(range(4854, 5000))
    actions = []
    for id in barrows:
        for charge in [1000, 1, 22500, 0]:
            actions.append(a('barrows', item=[id, 1, charge]))
    actions += [a('barrows', item=[4880, 3, 22499]), a('barrows', item=[4880, 1, -5]), a('barrows', item=[4880, 1, 65535]), a('barrows', item=[1163, 1])]
    cases.append(dict(items={}, actions=actions))
    # The death screen button.
    for button in [0, 1, 2]:
        cases.append(dict(items=ITEMS, players=[victim()], actions=[a('deathButton', player='p0', opcode=0, button=button, slot=-1, item=-1)]))
    # The grave type attribute.
    for value in [0, 5, 12, 13, -1, 'x', None]:
        attributes = {} if value is None else {'/save:gravetype': value}
        cases.append(dict(items=ITEMS, players=[victim(attributes=attributes)], actions=[a('graveType', player='p0'), a('allow', player='p0')]))


def requests():
    cases = []
    kept_cases(cases)
    death_cases(cases)
    grave_cases(cases)
    store_cases(cases)
    static_cases(cases)
    return cases
