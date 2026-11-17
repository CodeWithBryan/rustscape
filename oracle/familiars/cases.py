"""Designed cases for the familiars oracle (see generate.py).

A case sets up the recording world (the player, other nodes, NPC names, pouches, scrolls, the content classes of the
corpus by NPC id, item definitions and answers) and runs operations against the unchanged FamiliarManager, Familiar,
BurdenBeast, Forager, Pet and Pets. `sweep` marks the operation whose recorded calls fail one at a time in the second
pass. An operation that is expected to throw names the failure class in `expect`.
"""
import copy
import json

P = [3200, 3200, 0]
NEAR = [3201, 3200, 0]

NPC_NAMES = {
    '6829': 'Spirit wolf', '6830': 'Spirit wolf',        # a combat familiar: the next id has the same name
    '6825': 'Dreadfowl', '6826': 'Dreadfowl',            # combat, but its pouch is peaceful
    '6847': 'Spirit kalphite', '6848': 'Kalphite',       # not a combat familiar
    '6806': 'Thorny snail', '6807': 'Thorny snail',      # a beast of burden that would be a combat familiar
    '6818': 'Abyssal parasite', '6819': 'Abyssal lurker',
    '6808': 'Beaver', '6809': 'Beaver',
    '761': 'Kitten', '768': 'Cat', '774': 'Overgrown cat', '762': 'Kitten',
    '6969': 'Bulldog puppy', '6968': 'Bulldog', '3598': 'Clockwork cat', '6900': 'Baby dragon',
    '6915': 'Baby gecko', '6916': 'Gecko', '7018': 'Baby platypus',
}
POUCHES = {
    '12047': dict(level=4, cost=1, npc=6829, exp=4.8, peaceful=False, abyssal=False),
    '12043': dict(level=4, cost=1, npc=6825, exp=9.3, peaceful=True, abyssal=False),
    '12063': dict(level=25, cost=3, npc=6847, exp=31.6, peaceful=False, abyssal=False),
    '12019': dict(level=13, cost=2, npc=6806, exp=12.6, peaceful=False, abyssal=False),
    '12035': dict(level=54, cost=6, npc=6818, exp=94.8, peaceful=False, abyssal=True),
    '12021': dict(level=33, cost=4, npc=6808, exp=57.6, peaceful=True, abyssal=False),
    '12050': dict(level=50, cost=1, npc=6849, exp=10.0, peaceful=False, abyssal=False),
    '12051': dict(level=10, cost=1, npc=6850, exp=1.5, peaceful=False, abyssal=False),
    '12052': dict(level=13, cost=2, npc=6852, exp=12.6, peaceful=False, abyssal=False),
    '12053': dict(level=2, cost=1, npc=6856, exp=1.0, peaceful=False, abyssal=False),
}
SCROLLS = {
    '12047': dict(item=12425, exp=0.1), '12019': dict(item=12426, exp=0.2), '12063': dict(item=12446, exp=0.6),
}
CLASSES = {
    '6829': dict(type='plain', ticks=600, pouch=12047, cost=3, ids=[6829, 6830], boosts=[[23, 2.7], [5, 1.0], [23, 9.0]]),
    '6825': dict(type='plain', ticks=400, pouch=12043, cost=3, style=1, ids=[6825, 6826]),
    '6847': dict(type='plain', ticks=2200, pouch=12063, cost=6, ids=[6847, 6848]),
    '6806': dict(type='beast', ticks=1600, pouch=12019, cost=3, size=3, ids=[6806, 6807]),
    '6818': dict(type='beast', ticks=3000, pouch=12035, cost=1, size=7, style=2, ids=[6818, 6819]),
    '6808': dict(type='forager', ticks=2700, pouch=12021, cost=3, items=[[1511, 1], [1521, 2]], ids=[6808, 6809]),
    '6849': dict(type='plain', ticks=102, pouch=12050, cost=6, ids=[6849]),
    '6850': dict(type='plain', ticks=2, pouch=12051, cost=6, ids=[6850]),
    '6851': dict(type='plain', ticks=10, pouch=99999, cost=1, ids=[6851]),
    '6852': dict(type='beast', ticks=2, pouch=12052, cost=1, size=5, ids=[6852]),
    '6853': dict(type='plain', ticks=500, pouch=-1, cost=1, ids=[6853, 6829]),
    '6854': dict(type='forager', ticks=10, pouch=12021, cost=3, items=[], ids=[6854]),
    '6855': dict(type='beast', ticks=10, pouch=-1, cost=1, size=2, ids=[6855]),
    '6856': dict(type='plain', ticks=4, pouch=12053, cost=1, ids=[6856]),
}
ITEMS = {
    '1436': dict(name='Rune essence', value=4, tradeable=True, bankable=True),
    '1437': dict(name='Rune essence', value=4, tradeable=True, bankable=True, stackable=True),
    '7936': dict(name='Pure essence', value=4, tradeable=True, bankable=True),
    '1511': dict(name='Logs', value=4, tradeable=True, bankable=True),
    '995': dict(name='Coins', value=1, tradeable=True, bankable=True, stackable=True),
    '4151': dict(name='Abyssal whip', value=120001, tradeable=True, bankable=True),
    '6570': dict(name='Fire cape', value=1, tradeable=False, bankable=True),
    '2528': dict(name='LAMP', value=10, tradeable=True, bankable=False),
    '321': dict(name='Raw anchovies'), '1927': dict(name='Bucket of milk'), '2132': dict(name='Raw beef'),
}
NODES = {
    'v1': dict(kind='npc', id=90, location=[3203, 3200, 0]),
    'v2': dict(kind='player', location=[3202, 3201, 0]),
    'o1': dict(kind='other', id=995, location=[3200, 3201, 0]),
    'far': dict(kind='npc', id=91, location=[3230, 3200, 0]),
}


# A content class answers for its own NPC id and for every other id it lists (each listed once).
OWNED = {}
for _key, _spec in CLASSES.items():
    for _id in _spec['ids']:
        if str(_id) not in CLASSES:
            assert str(_id) not in OWNED, _id
            OWNED[str(_id)] = _key


def case(name, ops, group, **world):
    c = dict(name=name, group=group, player=dict(version=2, location=P), npcNames=NPC_NAMES, pouches=POUCHES, scrolls=SCROLLS,
             classes=CLASSES, items=ITEMS, nodes=NODES, answers={}, ops=ops)
    for k, v in world.items():
        if k == 'version':
            c['player'] = dict(c['player'], version=v)
        else:
            c[k] = v
    return copy.deepcopy(c)


def reg(*ids):
    return [dict(op='register', id=i) for i in ids]


def summoned(pouch=12047, npc=6829):
    return reg(npc) + [dict(op='summon', item=[pouch, 1, 0], pet=False)]


def j(value):
    return json.dumps(value)


CASES = []


def add(name, ops, group, sweep=None, **world):
    CASES.append((case(name, ops, group, **world), sweep))


# ------------------------------------------------------------------ Pets and PetDetails
ids = [1555, 1561, 1567, 7583, 7582, 7581, 14089, 15092, 7771, 7772, 12522, 12523, 12469, 13335, 14627, 12503, 12505, 1, -1, 0, 8851]
add('pets-tables', [dict(op='petsDump'), dict(op='petIds')] + [dict(op='petsForId', item=i) for i in ids] + [
    dict(op='petsNpcId', pet='CAT', item=1555), dict(op='petsNpcId', pet='CAT', item=1561), dict(op='petsNpcId', pet='CAT', item=1567),
    dict(op='petsNpcId', pet='BULLDOG', item=1567), dict(op='petsNpcId', pet='CLOCKWORK_CAT', item=-1),
    dict(op='petsNextStage', pet='CAT', item=1555), dict(op='petsNextStage', pet='CAT', item=1561), dict(op='petsNextStage', pet='CAT', item=1567),
    dict(op='petsNextStage', pet='BULLDOG', item=12523), dict(op='petsNextStage', pet='HELLCAT', item=7583),
    dict(op='petsKitten', pet='CAT_6', item=14089), dict(op='petsKitten', pet='CAT_6', item=14090), dict(op='petsKitten', pet='BULLDOG', item=12522),
    dict(op='petsGrownCat', pet='HELLCAT', item=7582), dict(op='petsGrownCat', pet='HELLCAT', item=7581), dict(op='petsGrownCat', pet='HELLCAT', item=7583),
    dict(op='petsGrownCat', pet='RAVEN', item=12485),
], 'pets')
add('pets-has-pet-none', [dict(op='petsHasPet')], 'pets')
add('pets-has-pet-grown', [dict(op='petsHasPet')], 'pets', answers={'contains_one:12519': True})
add('pets-has-pet-baby', [dict(op='petsHasPet')], 'pets', answers={'contains_one:12500': True, 'contains_one:1561': True})
add('pets-has-pet-overgrown', [dict(op='petsHasPet')], 'pets', answers={'contains_one:12511': True})

# ------------------------------------------------------------------ registration
add('register', reg(6829, 6806, 6808) + [dict(op='register', id=6829)], 'registry', sweep=0)
add('register-partial', reg(6829, 6853), 'registry', sweep=1)
add('register-missing-pouch', [dict(op='register', id=6851, expect='Null')], 'registry')
add('register-owned', [dict(op='register', id=6825, owner='p'), dict(op='construct', id=6826), dict(op='construct', id=6825, owner='v2')], 'registry')

# ------------------------------------------------------------------ summoning
add('summon', summoned(), 'summon', sweep=1)
add('summon-renew', summoned() + [dict(op='tick'), dict(op='summon', item=[12047, 1, 0], pet=False)], 'summon', sweep=2)
add('summon-follower', summoned() + [dict(op='summon', item=[12019, 1, 0], pet=False), dict(op='summon', item=[1555, 1, 0], pet=True)], 'summon')
add('summon-restricted', reg(6829) + [dict(op='summon', item=[12047, 1, 0], pet=False), dict(op='summon', item=[12047, 1, 0], pet=False)], 'summon',
    answers={'restricted_followers': True, 'is_locked': [False, True]})
add('summon-no-pouch', reg(6829) + [dict(op='summon', item=[4151, 1, 0], pet=False)], 'summon')
add('summon-levels', reg(6829) + [dict(op='summon', item=[12047, 1, 0], pet=False), dict(op='summon', item=[12047, 1, 0], pet=False)], 'summon',
    answers={'static_level': [3, 4], 'skill_level': 0})
add('summon-unregistered', [dict(op='summon', item=[12047, 1, 0], pet=False)], 'summon')
add('summon-too-big', reg(6829) + [dict(op='summon', item=[12047, 1, 0], pet=False)], 'summon', answers={'spawn_location': None})
add('summon-not-removed', reg(6829) + [dict(op='summon', item=[12047, 1, 0], pet=False)], 'summon', answers={'remove': False})
add('summon-wilderness', reg(6829, 6825, 6847) + [dict(op='summon', item=[12047, 1, 0], pet=False), dict(op='dismiss'),
                                                   dict(op='summon', item=[12063, 1, 0], pet=False), dict(op='transform')], 'summon',
    answers={'in_zone': True})
add('summon-hidden', reg(6829) + [dict(op='summon', item=[12047, 1, 0], pet=False)], 'summon',
    answers={'spawn_location': [[3201, 3200, 0], None]})
add('summon-call-variants', reg(6829) + [dict(op='summon', item=[12047, 1, 0], pet=False), dict(op='call'), dict(op='call')], 'summon',
    answers={'size': [1, 2, 1], 'combat_attacking': [False, True, False], 'is_renderable': [True, False, False], 'is_active': [True, False]})
add('summon-call-restricted', summoned() + [dict(op='call'), dict(op='call')], 'summon',
    answers={'restricted_followers': [False, True, True], 'is_locked': [True, False], 'spawn_location': [P, P, P, None]}, sweep=None)
add('summon-default-style', reg(6825) + [dict(op='summon', item=[12043, 1, 0], pet=False), dict(op='getAttackStyle'), dict(op='isPeaceful')], 'summon')

# ------------------------------------------------------------------ ticks
add('tick-drain', [dict(op='register', id=6849), dict(op='summon', item=[12050, 1, 0], pet=False)] + [dict(op='tick')] * 4, 'tick', sweep=3,
    answers={'text': ['', 'Woof!']})
# A drain of 2 over 4 ticks: the fraction reaches exactly 1.0 on the second tick and drains only on the third.
add('tick-drain-exact', [dict(op='register', id=6856), dict(op='summon', item=[12053, 1, 0], pet=False)] + [dict(op='tick')] * 4, 'tick')
add('tick-minute', [dict(op='register', id=6849), dict(op='summon', item=[12050, 1, 0], pet=False), dict(op='tick'), dict(op='tick')], 'tick',
    answers={'text': 'Woof!'})
add('tick-vanish', [dict(op='register', id=6850), dict(op='summon', item=[12051, 1, 0], pet=False), dict(op='tick'), dict(op='tick')], 'tick', sweep=3)
add('tick-vanish-beast', [dict(op='register', id=6852), dict(op='summon', item=[12052, 1, 0], pet=False), dict(op='tick'), dict(op='tick')], 'tick',
    sweep=3, answers={'is_empty': False, 'has_main_component': True, 'to_array': [[[1511, 1, 0], None, [995, 30, 2]]]})
add('tick-vanish-empty-beast', [dict(op='register', id=6852), dict(op='summon', item=[12052, 1, 0], pet=False), dict(op='tick'), dict(op='tick')], 'tick')
add('tick-assist', summoned() + [dict(op='tick')], 'tick', sweep=2,
    answers={'combat_attacking@p': True, 'combat_victim@p': 'v1', 'multi_zone': True})
add('tick-assist-attacker', summoned() + [dict(op='tick')], 'tick',
    answers={'in_combat@p': True, 'multi_zone': True}, attributes={'combat-attacker': None},
    nodes=dict(NODES, p2=dict(kind='npc', id=92, location=NEAR)))
add('tick-assist-attribute', summoned() + [dict(op='setAttribute', node='p', key='combat-attacker', value={'node': 'v2'}), dict(op='tick')], 'tick',
    answers={'in_combat@p': True, 'multi_zone': True})
add('tick-assist-self', summoned() + [dict(op='tick')], 'tick', answers={'combat_attacking@p': True, 'combat_victim@p': ['npc:2', 'npc:2'], 'multi_zone': True})
add('tick-assist-refused', summoned() + [dict(op='tick'), dict(op='tick'), dict(op='tick'), dict(op='setInvisible', node='v1', value=True), dict(op='tick')],
    'tick', answers={'combat_attacking@p': True, 'combat_victim@p': 'v1', 'multi_zone': True, 'multi_zone@v1': [False, True, True],
                     'combat_attacking@npc:2': [False, False, False, False, True, False, False, False]})
add('tick-assist-peaceful', reg(6825) + [dict(op='summon', item=[12043, 1, 0], pet=False), dict(op='tick')], 'tick',
    answers={'combat_attacking@p': True, 'combat_victim@p': 'v1', 'multi_zone': True})
add('tick-assist-beast', reg(6806) + [dict(op='summon', item=[12019, 1, 0], pet=False), dict(op='tick')], 'tick',
    answers={'combat_attacking@p': True, 'combat_victim@p': 'v1', 'multi_zone': True})
add('tick-recall', summoned() + [dict(op='setLocation', node='p', location=[3220, 3200, 0]), dict(op='tick'),
                                 dict(op='setLocation', node='p', location=[3250, 3200, 0]), dict(op='tick')], 'tick', sweep=3,
    answers={'spawn_location': [P, P, [3221, 3200, 0], None]})
add('tick-hidden-recall', reg(6829) + [dict(op='summon', item=[12047, 1, 0], pet=False), dict(op='setInvisible', node='npc:2', value=True)]
    + [dict(op='tick')] * 26, 'tick', answers={'has_pulse_running': True})
add('tick-follow', summoned() + [dict(op='tick'), dict(op='tick')], 'tick', answers={'has_pulse_running': [False, True]})
add('tick-forager', reg(6808) + [dict(op='summon', item=[12021, 1, 0], pet=False), dict(op='tick'), dict(op='tick'), dict(op='tick')], 'tick', sweep=4,
    answers={'ticks': [100, 500, 1000, 1000, 2000], 'random_range': 140, 'random': [2, 1, 9, 3, 0], 'forager_random': [11, 20]})
add('tick-forager-index', reg(6808) + [dict(op='summon', item=[12021, 1, 0], pet=False), dict(op='tick', expect='Index')], 'tick',
    answers={'ticks': [0, 0, 5000], 'random': [0, 2]})
add('tick-forager-full', reg(6808) + [dict(op='summon', item=[12021, 1, 0], pet=False), dict(op='tick')], 'tick',
    answers={'ticks': [0, 0, 5000], 'random': [0, 0], 'has_space_for': False})
add('tick-forager-empty', reg(6854) + [dict(op='summon', item=[12021, 1, 0], pet=False), dict(op='tick'), dict(op='produceRandom')], 'tick',
    pouches=dict(POUCHES, **{'12021': dict(POUCHES['12021'], npc=6854)}), answers={'ticks': [100, 1000]})

# ------------------------------------------------------------------ pets
add('pet-drop', [dict(op='summon', item=[1555, 1, 3], pet=True, delete=True), dict(op='hasPet'), dict(op='isUsingSummoning'), dict(op='tick')], 'pet', sweep=0)
add('pet-summon-kinds', [dict(op='summon', item=[8851, 1, 0], pet=True), dict(op='summon', item=[4151, 1, 0], pet=True),
                         dict(op='summon', item=[12469, 1, 0], pet=True), dict(op='summon', item=[7771, 1, 0], pet=True, delete=False),
                         dict(op='dismiss'), dict(op='summon', item=[7771, 1, 0], pet=True, delete=True)], 'pet',
    answers={'static_level': 50, 'remove': False})
add('pet-summon-existing', [dict(op='addDetails', item=1555, growth=40), dict(op='addDetails', item=1555, growth=60),
                            dict(op='summon', item=[1555, 1, 0], pet=True, delete=False), dict(op='sendConfiguration')], 'pet')
add('pet-hunger', [dict(op='parse', json=j({'petDetails': {'1555': [{'hunger': '74.99', 'growth': '10.0'}]}, 'currentPet': '1555'})), dict(op='login')]
    + [dict(op='tick')] * 3, 'pet', sweep=2)
add('pet-starving', [dict(op='parse', json=j({'petDetails': {'12522': [{'hunger': '98.5', 'growth': '1.0'}]}, 'currentPet': '12522'})), dict(op='login'),
                     dict(op='tick'), dict(op='setAttribute', node='p', key='petrate', value=2), dict(op='tick'), dict(op='tick', expect='Null')], 'pet', sweep=4)
add('pet-rates', [dict(op='summon', item=[12522, 1, 0], pet=True, delete=False), dict(op='setAttribute', node='p', key='petrate', value=0),
                  dict(op='tick'), dict(op='setAttribute', node='p', key='petrate', value=2), dict(op='tick')], 'pet')
add('pet-mastery', [dict(op='summon', item=[12522, 1, 0], pet=True, delete=False), dict(op='tick'), dict(op='tick')], 'pet',
    answers={'pet_mastery': [True, False]})
add('pet-grow', [dict(op='parse', json=j({'petDetails': {'1555': [{'hunger': '5.0', 'growth': '99.99'}]}, 'currentPet': '1555'})), dict(op='login'),
                 dict(op='tick'), dict(op='tick'), dict(op='tick')], 'pet', sweep=4)
add('pet-grow-grown', [dict(op='parse', json=j({'petDetails': {'12523': [{'hunger': '5.0', 'growth': '100.0'}]}, 'currentPet': '12523'})), dict(op='login'),
                       dict(op='tick'), dict(op='growNextStage')], 'pet')
add('pet-grow-detached', [dict(op='summon', item=[12522, 1, 0], pet=True, delete=False), dict(op='newPet', item=12488, id=6915, growth=100),
                          dict(op='growNextStage', fam='npc:2', expect='Null'), dict(op='addDetails', item=12488, growth=3),
                          dict(op='growNextStage', fam='npc:2')], 'pet', sweep=4)
add('pet-grow-not-pet', summoned() + [dict(op='newPet', item=1555, id=761, growth=100), dict(op='growNextStage', fam='npc:2', expect='Cast')], 'pet')
add('pet-grown-cat', [dict(op='summon', item=[1561, 1, 0], pet=True, delete=False), dict(op='tick')], 'pet')
add('pet-gecko-grow', [dict(op='summon', item=[12488, 1, 0], pet=True, delete=False), dict(op='setAttribute', node='p', key='petrate', value=2)]
    + [dict(op='tick')] * 2, 'pet')
add('pet-eat', [dict(op='summon', item=[1555, 1, 0], pet=True, delete=False), dict(op='eat', food=321), dict(op='eat', food=1927),
                dict(op='eat', food=995), dict(op='newPet', item=1556, id=762), dict(op='eat', food=321, fam='npc:2')], 'pet', sweep=1)
add('pet-eat-not-pet', summoned() + [dict(op='eat', food=321, expect='Cast')], 'pet')
add('pet-pickup', [dict(op='summon', item=[1555, 1, 0], pet=True, delete=False), dict(op='pickup'), dict(op='pickup'), dict(op='pickup'),
                   dict(op='pickup', expect='Null')], 'pet', sweep=3, answers={'free_slots': [0, 5, 5, 5], 'add': [False, True]})
add('pet-pickup-none', [dict(op='pickup', expect='Null')], 'pet')
add('pet-pickup-familiar', summoned() + [dict(op='pickup', expect='Cast')], 'pet')
add('pet-morph', [dict(op='summon', item=[1555, 1, 0], pet=True, delete=False), dict(op='setHasWarned', value=1),
                  dict(op='morphPet', item=[1561, 1, 0], delete=False, location=[3205, 3205, 0], hunger=40.5, growth=-1)], 'pet', sweep=2)
add('pet-morph-familiar', summoned() + [dict(op='morphPet', item=[1561, 1, 0], delete=False, location=None, hunger=-1, growth=3, expect='Cast')], 'pet')
add('pet-run-away-clockwork', [dict(op='parse', json=j({'petDetails': {'7771': [{'hunger': '150.0', 'growth': '100.0'}]}, 'currentPet': '7771'})),
                               dict(op='login'), dict(op='tick'), dict(op='isPeaceful', expect='Null'), dict(op='sendConfiguration')], 'pet')
add('pet-details', [dict(op='addDetails', item=1555), dict(op='addDetails', item=12522, growth=5), dict(op='removeDetails', item=1555),
                    dict(op='removeDetails', item=1555), dict(op='removeDetails', item=4151, expect='Null')], 'pet')

# ------------------------------------------------------------------ parse and login
add('parse-v2', [dict(op='parse', json=j({'petDetails': {'1555': [{'hunger': '1.5', 'growth': '2.5'}, {'hunger': '3.0', 'growth': '120'}],
                                                         '12522': [], '7581': [{'hunger': '-4', 'growth': '-5'}], '13335': [{'hunger': '1E2', 'growth': '0.1'}]},
                                          'currentPet': '1555'})), dict(op='login')], 'parse', sweep=0)
add('parse-v2-empty-current', [dict(op='parse', json=j({'petDetails': {'12522': []}, 'currentPet': '12522'}))], 'parse')
add('parse-v2-missing-current', [dict(op='parse', json=j({'petDetails': {}, 'currentPet': '12522'}), expect='Null')], 'parse')
add('parse-v2-not-pet', [dict(op='parse', json=j({'petDetails': {'4151': []}, 'currentPet': '4151'}), expect='Null')], 'parse')
add('parse-familiar', reg(6806) + [dict(op='parse', json=j({'petDetails': {}, 'familiar': {'originalId': '6806', 'ticks': '77', 'specialPoints': '33',
                                                                                          'inventory': [{'slot': '0', 'id': '1511', 'amount': '1'}], 'lifepoints': '9'}})),
                                   dict(op='login')], 'parse', sweep=1)
add('parse-familiar-no-inventory', reg(6829) + [dict(op='parse', json=j({'petDetails': {}, 'familiar': {'originalId': '6829', 'ticks': '5', 'specialPoints': '61', 'lifepoints': '1'}}))], 'parse')
add('parse-familiar-not-beast', reg(6829) + [dict(op='parse', json=j({'petDetails': {}, 'familiar': {'originalId': '6829', 'ticks': '5', 'specialPoints': '6', 'inventory': [], 'lifepoints': '1'}}), expect='Cast')], 'parse')
add('parse-familiar-unregistered', [dict(op='parse', json=j({'petDetails': {}, 'familiar': {'originalId': '6829'}}), expect='Null')], 'parse')
add('parse-familiar-bad-ticks', reg(6829) + [dict(op='parse', json=j({'petDetails': {}, 'familiar': {'originalId': '6829', 'ticks': 'x'}}), expect='Number')], 'parse')
add('parse-familiar-inventory-type', reg(6806) + [dict(op='parse', json=j({'petDetails': {}, 'familiar': {'originalId': '6806', 'ticks': '1', 'specialPoints': '1', 'inventory': {}}}), expect='Cast')], 'parse')
add('parse-familiar-null', [dict(op='parse', json=j({'petDetails': {}, 'familiar': None}), expect='Null')], 'parse')
add('parse-number', [dict(op='parse', json=j({'petDetails': {}, 'currentPet': 'cat'}), expect='Number')], 'parse')
add('parse-key-number', [dict(op='parse', json=j({'petDetails': {'1555': [], 'x': []}}), expect='Number')], 'parse')
add('parse-null-hunger', [dict(op='parse', json=j({'petDetails': {'1555': [{'growth': '1'}]}}), expect='Null')], 'parse')
add('parse-bad-hunger', [dict(op='parse', json=j({'petDetails': {'1555': [{'hunger': 'abc', 'growth': '1'}]}}), expect='Number')], 'parse')
add('parse-details-type', [dict(op='parse', json=j({'petDetails': []}), expect='Cast')], 'parse')
add('parse-details-missing', [dict(op='parse', json=j({}), expect='Null')], 'parse')
add('parse-list-type', [dict(op='parse', json=j({'petDetails': {'1555': {}}}), expect='Cast')], 'parse')
add('parse-entry-type', [dict(op='parse', json=j({'petDetails': {'1555': [5]}}), expect='Cast')], 'parse')
add('parse-v1', [dict(op='parse', json=j({'petDetails': [{'petId': str(1555 << 16 | 7), 'hunger': '3.5', 'growth': '50'},
                                                         {'petId': '12522', 'stage': '1', 'hunger': '1', 'growth': '2'},
                                                         {'petId': '1555', 'stage': '2', 'hunger': '0', 'growth': '0'},
                                                         {'petId': '1556', 'stage': '0', 'hunger': '9', 'growth': '9'}],
                                          'currentPet': str(12523 << 16 | 99)}))], 'parse', version=1)
add('parse-v1-no-next', [dict(op='parse', json=j({'petDetails': [{'petId': '12522', 'stage': '2', 'hunger': '1', 'growth': '2'}]}), expect='Null')], 'parse', version=1)
add('parse-v1-not-pet', [dict(op='parse', json=j({'petDetails': [{'petId': '4151', 'stage': '1', 'hunger': '1', 'growth': '2'}]}), expect='Null')], 'parse', version=0)
add('parse-v1-hash-unknown', [dict(op='parse', json=j({'petDetails': [{'petId': '5', 'hunger': '1', 'growth': '2'}]}), expect='Null')], 'parse', version=1)
add('parse-v1-missing', [dict(op='parse', json=j({}), expect='Null')], 'parse', version=1)
add('parse-v1-small-current', [dict(op='parse', json=j({'petDetails': [], 'currentPet': '1555'})), dict(op='login')], 'parse', version=1,
    answers={'wolf_whistle_complete': True})
add('parse-again', [dict(op='addDetails', item=1555, growth=7), dict(op='parse', json=j({'petDetails': {'12522': [{'hunger': '1', 'growth': '1'}], '1555': []}}))], 'parse')
add('login-none', [dict(op='login')], 'login', answers={'wolf_whistle_complete': True})
add('login-familiar', reg(6829) + [dict(op='parse', json=j({'petDetails': {}, 'familiar': {'originalId': '6829', 'ticks': '50', 'specialPoints': '10', 'lifepoints': '1'}})),
                                   dict(op='login')], 'login', sweep=2)

# ------------------------------------------------------------------ attack rules and specials
add('attackable', summoned() + [
    dict(op='isAttackable', entity='p', style='MELEE', message=True), dict(op='isAttackable', entity='p', style='MELEE', message=False),
    dict(op='isAttackable', entity='v1', style='RANGE', message=True), dict(op='isAttackable', entity='v2', style='MAGIC', message=True),
    dict(op='isAttackable', entity='v2', style='MAGIC', message=True), dict(op='isAttackable', entity='v2', style='MAGIC', message=True),
    dict(op='isAttackable', entity='v2', style='MAGIC', message=True), dict(op='isAttackable', entity='v2', style='MAGIC', message=True),
    dict(op='isAttackable', entity='v2', style='MAGIC', message=False)], 'combat', sweep=9,
    answers={'skull_wilderness@v2': [False, True, True, True, True, True, True], 'skull_wilderness@p': [False, True, True, True, True, True],
             'is_attackable@p': [False, True, True, True, True], 'multi_zone@v2': [False, True, True, True],
             'multi_zone@npc:2': [False, True, True], 'npc_is_attackable@npc:2': [True, False]})
add('continue-attack', summoned() + [
    dict(op='continueAttack', entity='p', style='MELEE', message=True), dict(op='continueAttack', entity='v1', style='MELEE', message=True),
    dict(op='continueAttack', entity='p', style='MELEE', message=True), dict(op='continueAttack', entity='v2', style='RANGE', message=True),
    dict(op='continueAttack', entity='v2', style='RANGE', message=True), dict(op='continueAttack', entity='v2', style='RANGE', message=True),
    dict(op='continueAttack', entity='v2', style='RANGE', message=True), dict(op='continueAttack', entity='v2', style='RANGE', message=False),
    dict(op='continueAttack', entity='v2', style='RANGE', message=True), dict(op='continueAttack', entity='v1', style='MELEE', message=True)], 'combat',
    answers={'combat_victim@npc:2': [None, None, 'p', 'v2', 'v2', 'v2', 'v2', 'v2', 'v2', 'v1'], 'skull_wilderness@v2': [False, True, True, True, True, True],
             'skull_wilderness@p': [False, True, True, True], 'is_attackable@p': [False, True, True], 'multi_zone@p': [False, False, True]})
add('can-attack', summoned() + [
    dict(op='canAttack', target='v1'), dict(op='canAttack', target='far', message=True), dict(op='canAttack', target='far', message=False),
    dict(op='canAttack', target='v2'), dict(op='canAttack', target='v2', message=False), dict(op='canAttack', target='v1', message=True),
    dict(op='canAttack', target='v1', message=False), dict(op='canAttack', target='v2', message=True)], 'combat', sweep=9,
    answers={'is_attackable@v2': [True, True, True], 'npc_is_attackable@v1': [False, True, True, False, True], 'multi_zone@p': [False, False, True],
             'interaction_locked@v1': [False, True, False, False], 'combat_style@p': ['RANGE', 'MAGIC']})
add('can-attack-not-combat', reg(6847) + [dict(op='summon', item=[12063, 1, 0], pet=False), dict(op='canAttack', target='v1'), dict(op='isCombatFamiliar')], 'combat')
add('combat-special', summoned() + [
    dict(op='canCombatSpecial', target='v1'), dict(op='canCombatSpecial', target='v1', message=False), dict(op='isOwnerAttackable'),
    dict(op='canCombatSpecial', target='v1', message=True), dict(op='canCombatSpecial', target='v1'), dict(op='canCombatSpecial', target='v1'),
    dict(op='canCombatSpecial', target='v1')], 'combat', sweep=8,
    answers={'npc_is_attackable@v1': [False, True], 'combat_attacking@p': [False, False, False, True], 'in_combat@p': [False, True],
             'combat_attacking@npc:2': [False, True, False], 'ticks': [100, 10, 10, 10], 'npc_task@v1': [None, 99, 99, 5, 5]},
    nodes=dict(NODES, v1=dict(NODES['v1'], attributes={})))
add('combat-special-delay', summoned() + [dict(op='setAttribute', node='npc:2', key='special-delay', value=150), dict(op='canCombatSpecial', target='v1')], 'combat',
    answers={'combat_attacking@p': True})
add('special-move', summoned() + [
    dict(op='specialMove', node='npc:1'), dict(op='specialMove', node='v1', iface=662, component=51, item=[12425, 1, 4]),
    dict(op='specialMove', node=None), dict(op='updateSpecialPoints', diff=-10), dict(op='specialMove', node='v1')], 'special', sweep=3,
    answers={'ticks': 200, 'special_move': [True, False, True]})
add('special-move-refusals', summoned() + [dict(op='updateSpecialPoints', diff=58), dict(op='specialMove', node='v1'), dict(op='updateSpecialPoints', diff=-20),
                                           dict(op='specialMove', node='v1'), dict(op='setLocation', node='p', location=[3220, 3200, 0]), dict(op='specialMove', node='v1')],
    'special', answers={'contains': [False, True]})
add('special-move-no-scroll', reg(6825) + [dict(op='summon', item=[12043, 1, 0], pet=False), dict(op='specialMove', node='v1')], 'special')
add('special-move-infinite', summoned() + [dict(op='setAttribute', node='p', key='infinite-special-move', value=True), dict(op='specialMove', node='v1')], 'special')
add('special-move-reentrant', summoned() + [dict(op='specialMove', node='v1')], 'special',
    hooks=[dict(event='special_move', at=1, actions=[dict(op='run', **{'do': dict(op='dismiss')})])])
add('special-target', [dict(op='specialTarget', node='v1'), dict(op='specialTarget', node=None), dict(op='specialTarget', node='o1', expect='Cast')], 'special')
add('familiar-hit', summoned() + [dict(op='sendHit', target='v1', max=12), dict(op='pulse', index=1), dict(op='sendHit', target='far', max=5, gfx=1376),
                                  dict(op='pulse', index=2), dict(op='sendHit', target='v2', max=3), dict(op='pulse', index=3)], 'special', sweep=3,
    answers={'accurate_impact': [True, False, True], 'randomize': [7, 2], 'npc_task@v1': [9, 9], 'npc_task@far': [1, 1], 'skill_level': [99, 5, 50],
             'state_style': ['MAGIC', 'MELEE']})
add('familiar-misc', summoned() + [
    dict(op='projectile', target='v1', id=1333), dict(op='swingHandler', swing=True), dict(op='setCombatHandler', handler='handler:special'),
    dict(op='swingHandler', swing=False), dict(op='setCombatHandler', handler=None), dict(op='swingHandler', swing=False),
    dict(op='regionInactivity'), dict(op='regionInactivity'), dict(op='refreshTimer'), dict(op='isCharged'), dict(op='charge'), dict(op='isCharged'),
    dict(op='setCharged', value=False), dict(op='familiarBoost', skill=23), dict(op='familiarBoost', skill=5), dict(op='familiarBoost', skill=1),
    dict(op='getBoost', skill=23), dict(op='visualize'), dict(op='startFollowing'), dict(op='pulse', index=1), dict(op='spawnLocation'),
    dict(op='updateSpecialPoints', diff=-30), dict(op='updateSpecialPoints', diff=100), dict(op='isBurdenBeast'), dict(op='isPeaceful'),
    dict(op='setOwner', owner='v2'), dict(op='sendConfiguration')], 'familiar', answers={'spawn_location': [P, P, None]})
add('familiar-death', summoned() + [dict(op='finalizeDeath'), dict(op='hasFamiliar'), dict(op='getBoost', skill=23), dict(op='isOwner', fam='npc:1')], 'familiar')
add('manager-misc', summoned() + [
    dict(op='isOwner', fam='npc:2'), dict(op='newFamiliar', id=6847), dict(op='isOwner', fam='npc:3'), dict(op='adjustBattleState', victim='v1'),
    dict(op='setHasPouch', value=True), dict(op='isUsingSummoning'), dict(op='setCombatLevel', value=12), dict(op='dismiss'), dict(op='isUsingSummoning'),
    dict(op='setHasPouch', value=False), dict(op='isUsingSummoning'), dict(op='adjustBattleState', victim='v1'), dict(op='isOwner', fam='npc:3'),
    dict(op='morphFamiliar', location=P, expect='Null'), dict(op='spawnFamiliar', expect='Null'), dict(op='setFamiliar', fam='npc:3'),
    dict(op='spawnFamiliar'), dict(op='morphFamiliar', location=[3210, 3210, 1])], 'manager')
add('init-at', [dict(op='newFamiliar', id=6829), dict(op='setFamiliar', fam='npc:1'), dict(op='initAt', fam='npc:1', location=None, call=False),
                dict(op='initAt', fam='npc:1', location=[3199, 3199, 0], call=True)], 'familiar', sweep=2,
    answers={'in_zone': [False, True]})

# ------------------------------------------------------------------ beasts of burden and foragers
add('allowed', reg(6806, 6818) + [dict(op='summon', item=[12019, 1, 0], pet=False)] + [
    dict(op='isAllowed', item=[i, 1]) for i in (4151, 6570, 1436, 7936, 2528, 1511)] + [dict(op='dismiss'), dict(op='summon', item=[12035, 1, 0], pet=False)] + [
    dict(op='isAllowed', item=[i, 1], owner='v2') for i in (1436, 1437, 7936, 1511, 2528)], 'burden', sweep=9)
add('allowed-no-pouch', [dict(op='newFamiliar', id=6855), dict(op='isAllowed', fam='npc:1', item=[1511, 1], expect='Null'),
                         dict(op='isPeaceful', fam='npc:1', expect='Null')], 'burden')
add('transfer', reg(6806) + [dict(op='summon', item=[12019, 1, 0], pet=False),
                             dict(op='transfer', item=[1511, 1, 3], amount=5, withdraw=False), dict(op='transfer', item=[995, 500, 0], amount=1000, withdraw=False),
                             dict(op='transfer', item=None, amount=1, withdraw=True), dict(op='transfer', item=[1511, 1, 1], amount=2, withdraw=True),
                             dict(op='transfer', item=[1511, 1, -1], amount=1, withdraw=True), dict(op='transfer', item=[995, 9, 0], amount=0, withdraw=False),
                             dict(op='transfer', item=[995, 9, 0], amount=0, withdraw=True), dict(op='transfer', item=[4151, 1, 0], amount=1, withdraw=False)],
    'burden', sweep=2,
    answers={'get_amount': [3, 400, 2, 1, 5, 5], 'maximum_add': [2, 10000, 5, 1, 0, 0], 'remove': [True, False, True]})
add('transfer-forager', reg(6808) + [dict(op='summon', item=[12021, 1, 0], pet=False), dict(op='transfer', item=[1511, 1, 0], amount=1, withdraw=False),
                                     dict(op='transfer', item=[1511, 1, 0], amount=1, withdraw=True)], 'burden', answers={'get_amount': 1, 'maximum_add': 1})
add('transfer-not-beast', summoned() + [dict(op='transfer', item=[1511, 1, 0], amount=1, withdraw=True, expect='Cast')], 'burden')
add('withdraw-all', reg(6806) + [dict(op='summon', item=[12019, 1, 0], pet=False), dict(op='withdrawAll')], 'burden', sweep=2,
    answers={'capacity@container:2': 3, 'get@container:2': [[1511, 1, 0], None, [995, 300, 2]], 'maximum_add': [5, 100]})
add('open-interface', reg(6806, 6808) + [dict(op='summon', item=[12019, 1, 0], pet=False), dict(op='openInterface'), dict(op='closeEvent', index=0),
                                         dict(op='dismiss'), dict(op='summon', item=[12021, 1, 0], pet=False), dict(op='openInterface'), dict(op='openInterface'),
                                         dict(op='openInterface', expect='Null')], 'burden', sweep=2,
    answers={'to_array': [[[1511, 1, 0]], [None, [995, 5, 1]], [], []], 'item_count': [0, 0, 2, 3], 'open': [True, True, False]})
add('listener', reg(6806) + [dict(op='summon', item=[12019, 1, 0], pet=False), dict(op='listenerUpdate', items=[[1511, 1, 0], None], slots=[0, 1]),
                             dict(op='listenerRefresh')], 'burden', sweep=3, answers={'to_array': [[[995, 1, 0]]], 'capacity': 3})
add('produce', reg(6808) + [dict(op='summon', item=[12021, 1, 0], pet=False), dict(op='produceItem', item=[1511, 1]), dict(op='produceItem', item=[1511, 3]),
                            dict(op='produceItem', item=[1511, 3]), dict(op='produceRandom'), dict(op='setRandomPassive'), dict(op='passiveAction'),
                            dict(op='poisonImmune')], 'burden', sweep=5,
    answers={'has_space_for': [True, True, False, True], 'add': [True, False, True], 'random': 1, 'random_range': 300})
add('beast-dismiss', reg(6806) + [dict(op='summon', item=[12019, 1, 0], pet=False), dict(op='dismissFamiliar')], 'burden', sweep=2,
    answers={'has_main_component': True, 'to_array': [[[1511, 1, 0], None, [995, 30, 2]]]})

# ------------------------------------------------------------------ reentrant services
add('tick-reentrant-text', [dict(op='register', id=6849), dict(op='summon', item=[12050, 1, 0], pet=False), dict(op='tick'), dict(op='tick')], 'reentrant',
    answers={'text': 'Hi'}, hooks=[dict(event='text', at=1, actions=[dict(op='run', **{'do': dict(op='dismiss')})])])
add('tick-reentrant-familiar-tick', summoned() + [dict(op='tick')], 'reentrant',
    hooks=[dict(event='familiar_tick', at=1, actions=[dict(op='run', **{'do': dict(op='summon', item=[12047, 1, 0], pet=False)})])])
add('construct-reentrant', reg(6829) + [dict(op='summon', item=[12047, 1, 0], pet=False)], 'reentrant',
    hooks=[dict(event='construct', at=1, actions=[dict(op='run', **{'do': dict(op='summon', item=[1555, 1, 0], pet=True, delete=False)})])])
add('hook-fail', summoned() + [dict(op='tick')], 'reentrant', hooks=[dict(event='familiar_tick', at=1, actions=[dict(op='fail')])])

NEEDED_EVENTS = [
    'new_npc', 'npc_definition_name', 'set_invisible', 'npc_init', 'npc_clear', 'npc_transform', 'pulse_run', 'pulse_clear', 'has_pulse_running',
    'submit_pulse', 'face', 'send_chat', 'graphics', 'size', 'is_renderable', 'walking_update', 'masks_prepare', 'set_teleport_location', 'in_zone',
    'restricted_followers', 'is_locked', 'interaction_locked', 'get_attribute', 'set_attribute', 'increment_attribute', 'is_attackable',
    'npc_is_attackable', 'npc_swing_handler', 'combat_attacking', 'combat_victim', 'combat_attack', 'combat_next_attack', 'combat_style', 'in_combat',
    'multi_zone', 'skull_wilderness', 'npc_task', 'is_active', 'spawn_location', 'skill_level', 'static_level', 'update_level', 'add_experience',
    'packet_message', 'player_message', 'api_message', 'send_dialogue', 'set_varp', 'set_varbit', 'play_audio', 'visualize', 'animate',
    'appearance_sync', 'wolf_whistle_complete', 'pet_mastery', 'open_tab', 'set_viewed_tab', 'open_info_bars', 'remove_tabs', 'has_main_component',
    'close', 'open', 'set_close_event', 'open_single_tab', 'close_single_tab', 'generate_items', 'container_packet', 'battle_state', 'accurate_impact',
    'set_estimated_hit', 'neutralize_hits', 'set_poison_damage', 'state_style', 'handle_impact', 'projectile', 'ticks', 'random', 'random_range',
    'randomize', 'new_container', 'register_burden_listener', 'get_amount', 'maximum_add', 'replace', 'add', 'remove', 'to_array', 'get', 'capacity',
    'item_count', 'is_empty', 'clear', 'shift', 'refresh', 'update', 'free_slots', 'has_space_for', 'contains', 'contains_one', 'parse_container',
    'item_stackable', 'item_tradeable', 'item_bankable', 'item_value', 'item_name', 'item_definition_name', 'ground_item', 'pouch', 'scroll',
    'construct', 'special_move', 'ids', 'configure', 'text', 'familiar_tick', 'adjust_player_battle', 'forager_random', 'log',
]
# Failure classes an operation must name in `expect`.
UNEXPECTED = ('Null', 'Cast', 'Number', 'Index')


def cases():
    return CASES


def injects(row):
    return any(a.get('op') == 'fail' for h in row.get('hooks', []) for a in h['actions'])


def groups(cases):
    out = {}
    for c in cases:
        out[c['group']] = out.get(c['group'], 0) + 1
    return dict(sorted(out.items()))
