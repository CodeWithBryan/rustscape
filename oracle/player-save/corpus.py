"""Authored inputs for the player save oracle. Expected values come only from the JVM."""
import copy
import json
import random

INT_MIN, INT_MAX = -2**31, 2**31 - 1
LONG_MIN, LONG_MAX = -2**63, 2**63 - 1
SAVER_SECTIONS = ['saveVersion', 'saveCoreData', 'saveSkills', 'saveSettings', 'saveQuests', 'saveAppearance', 'saveSpellbook', 'saveSavedData', 'saveActivityData', 'saveQuestData',
    'saveGlobalData', 'saveAutocast', 'savePlayerMonitor', 'saveMusicPlayer', 'saveFamiliarManager', 'saveBankPinData', 'saveHouseData', 'saveAchievementData', 'saveIronManData',
    'saveEmoteData', 'saveStatManager', 'saveAttributes', 'savePouches', 'savePOHStorageRoom']
# Each parser section and the top-level keys it reads.
PARSER_SECTIONS = dict(parseVersion=['version'], parseCore=['core_data'], parseAttributes=['attributes'], parseSkills=['skills', 'totalEXP', 'exp_multiplier', 'milestone'], parseSettings=['settings'],
    parseQuests=['quests'], parseAppearance=['appearance'], parseGrave=['grave_type'], parseVarps=['varps'], parseSpellbook=['spellbook'], parseSavedData=['activityData', 'questData', 'globalData'],
    parseAutocastSpell=['autocastSpell'], parseFarming=[], parseConfigs=['configs'], parseMonitor=[], parseMusic=['unlockedMusic'], parseFamiliars=['familiarManager'], parseBankPin=['bankPinManager'],
    parseHouse=['houseData'], parseIronman=['ironManMode'], parseEmoteManager=['emoteData'], parseStatistics=['statistics'], parseAchievements=['achievementDiaries'], parsePouches=['pouches'],
    parsePOHStorable=['pohstorage'])
OWNER = {key: name for name, keys in PARSER_SECTIONS.items() for key in keys}
WRONG = [None, True, False, 0, -7, 1.5, 'x', '5', [], {}, [None], [False], ['5'], [{}], {'a': 1}]
TIMERS = {'p': {'id': 'persist', 'interval': 5, 'class': 'PersistentTimer'}, 'q': {'id': 'other', 'interval': 2, 'class': 'ChildPersistent'}}
REGISTRY = {'persist': 'p', 'other': 'q'}
STRINGS = ['', ' ', 'plain', 'quote"inside', 'back\\slash', 'slash/slash', 'tab\there', 'line\nfeed\r', '\b\f', '\x00\x01\x1f', '\x7f\x80\x9f\xa0', 'caf\u00e9', '\u2000\u2028\u2029\u20ff\u2100',
    '\u65e5\u672c\u8a9e', '\U0001f600 emoji', '</script>', 'null', 'true', '123', '\ufeff bom', '{"json":1}', 'a' * 300]


def op(_op, **args):
    return dict(op=_op, **args)


def quiet(_op, **args):
    return dict(op=_op, quiet=True, **args)


def J(value):
    return json.dumps(value, separators=(',', ':'), ensure_ascii=False)


def row(slot=0, id=1, amount=3, charge=1000):
    return dict(slot=str(slot), id=str(id), amount=str(amount), charge=str(charge))


WORLD = dict(
    quests=dict(points=7, list=[[1, 10], [17, 100], [0, 3], [33, 5], [16, 2], [-1, -1]]),
    spells=[[0, 5, True], [1, 3, True], [2, 9, False], [0, -1, True]],
    autocast=[1, 3],
    songs=[[62, 0], [76, 16], [2, 17], [333, 1], [151, 32], [7, 33], [-5, -9]],
    unlocked=[76, 2, 62, 151, 7],
    emotes=list(range(22)) + [30, 44],
    diaries=[dict(type=0, started=[True, False, False], completed=[[True, False] * 5, [False] * 12, [True] * 8], rewarded=[False, True, False]),
        dict(type=5, started=[False, False, True], completed=[[], [True], [False, False]], rewarded=[True, True, True]), dict(type=2, started=[], completed=[], rewarded=[])],
    house=dict(location=3, style=2, crest=11, servant=dict(type=4, uses=6, item=[5, 20], greet=True),
        rooms=[dict(z=1, x=3, y=4, properties=7, rotation=2, hotspots=[-1, 0, 3, -1, 5]), dict(z=0, x=7, y=7, properties=0, rotation=0, hotspots=[]), dict(z=1, x=3, y=3, properties=19, rotation=3, hotspots=[-2, INT_MAX]),
            dict(z=3, x=0, y=0, properties=INT_MIN, rotation=-1, hotspots=[0])]),
    pets=[[1555, [['0.0', '100.0'], ['12.5', '3.25']]], [7583, [['1.0E10', '0.1']]], [12, []], [17, [['NaN', '-0.0'], ['Infinity', '1.0E-5'], ['0.001', '1.0E7'], ['4.9E-324', '1.7976931348623157E308']]]],
    familiar=dict(originalId=6873, ticks=1234, specialPoints=45, lifepoints=17, burden=[[0, 1, 5, 1000], [3, 2, 1, 1000], [29, 7, INT_MAX, 0]]),
    pouches=[['pouches', '[{"id":"5509","container":[{"itemId":"1436","amount":"3"}],"charges":"9","currentCap":"3"}]']],
    poh='{"containers":{"toys":{"x":1,"y":[2.5,"t",null,true]},"capes":{}}}',
    hooks=[['slayer', '{"points":"5","task":null}'], ['custom', '[1,2.5,"t",null,true,{"k":[]}]']],
)


def rich_save(base):
    """A save that exercises every section the parser reads, built on the JVM's own default save."""
    save = copy.deepcopy(base)
    core = save['core_data']
    core['inventory'] = [row(0, 1, 3), row(5, 2, 1, 999), row(27, 7, 2147483647, 0)]
    core['bank'] = [row(0, 3, 99), row(1, 4, 1), row(495, 5, 12)]
    core['bankSecondary'] = [row(4, 6, 7)]
    core['equipment'] = [row(3, 3), row(13, 0, 1)]
    core['bankTabs'] = [dict(index='2', startSlot='14'), dict(index='10', startSlot='-1')]
    core['bankTabsSecondary'] = [dict(index='0', startSlot='9')]
    core['useSecondaryBank'] = True
    core['location'] = '3200,3201,2'
    core['varp'] = [dict(index='64', value='3'), None, dict(index='0', value='7'), dict(index='1048', value='-9')]
    core['timers'] = {'persist': {'ticksLeft': '9'}}
    for i, skill in enumerate(save['skills']):
        skill.update(static=str(1 + i * 4), dynamic=str(i * 5 - 3), experience=str(i * 1234.5))
    save['skills'][3]['lifepoints'] = '77'
    save['skills'][5]['prayerPoints'] = '12.25'
    save['totalEXP'] = '123456.75'
    save['exp_multiplier'] = '5.0'
    save['milestone'] = dict(combatMilestone='3', skillMilestone='0')
    save['settings'].update(brightness='4', musicVolume='1', soundEffectVolume='2', areaSoundVolume='3', publicChatSetting='1', privateChatSetting='2', clanChatSetting='3', tradeSetting='1', assistSetting='2',
        runEnergy='37.5', specialEnergy='65', attackStyle='2', singleMouse=True, disableChatEffects=True, splitPrivate=True, acceptAid=True, runToggled=True, retaliation=False)
    save['attributes'] = [dict(type='int', key='count', value='5'), dict(type='bool', key='flag', value=True), dict(type='long', key='time', value='9007199254740993'), dict(type='short', key='small', value='-32768'),
        dict(type='str', key='text', value='hello "world"'), dict(type='byte', key='b', value='/w=='), dict(type='location', key='home', value='[3222, 3218, 0]'),
        {'type': 'int', 'key': 'timed', 'value': '1', 'expirable': True, 'expiration-time': '5000'}, {'type': 'str', 'key': 'gone', 'value': 'x', 'expirable': True, 'expiration-time': '10'}]
    save['appearance'] = dict(gender='1', appearance_cache=[dict(look=str(40 + i), color=str(i)) for i in range(7)])
    save['spellbook'] = '193'
    save['bankPinManager'] = dict(pin='1234', longRecovery=True, status='2', pendingDelay='99999', tryDelay='88888')
    save['ironManMode'] = '2'
    save['emoteData'] = ['0', '25', '44', '25', '21']
    save['statistics'] = [dict(index='1', value='2')]
    save['grave_type'] = '5'
    save['autocastSpell'] = dict(book='1', spellId='3')
    save['unlockedMusic'] = ['76', '2', '76', '-5']
    save['configs'] = [dict(index='300', value='1000'), dict(index='1048', value='3'), dict(index='64', value='-1')]
    save['varps'] = [dict(index='1', bitArray=[dict(offset='0', value='1'), dict(offset='4', value='3'), dict(offset='33', value='1')]), dict(index='2', bitArray=[])]
    save['activityData'].update(pestPoints='250', barrowBrothers=[True, False, True, False, True, False], kolodionStage='2', godCasts=['1', '2', '3'], lastBorkBattle='1700000000000', elnockSupplies=True, fogRating='-4')
    save['questData'].update(dragonSlayerPlanks='3', talkedDrezel=True, desertTreasureNode=[dict(id='1', amount='5'), dict(id='2', amount='0')], witchsExperimentStage='4')
    save['globalData'].update(homeTeleportDelay='1700000000001', lumbridgeRope=True, assistExperience=['1.5', '0.0', '1.0E10', '0.001', '123456.789', '0.0', '0.0', '0.0', '2.0'], chatPing='5', zafAmount='3',
        rcDecays=['1', '2', '3'], godBooks=[True, False, True], bossCounters=[str(i) for i in range(25)], doubleExp='42', macroDisabled=True)
    return save


def remove(value, path):
    out = copy.deepcopy(value)
    target = out
    for key in path[:-1]:
        target = target[key]
    del target[path[-1]]
    return out


def replace(value, path, new):
    out = copy.deepcopy(value)
    target = out
    for key in path[:-1]:
        target = target[key]
    target[path[-1]] = copy.deepcopy(new)
    return out


def only(save, name):
    """The part of a save one parser section reads."""
    return {key: save[key] for key in PARSER_SECTIONS[name] if key in save}


def seed_requests():
    return [dict(timers={}, config={}, actions=[op('populate')])]


def requests(seed):
    base_text = seed[0]['expected'][0]['result']['text']
    base = json.loads(base_text)
    rich = rich_save(base)
    rich_text = J(rich)
    rich_config = dict(clock=1000, world=WORLD)
    # Most section tests only need the tables the parser looks things up in.
    lean_config = dict(clock=1000, world=dict(songs=WORLD['songs'], spells=WORLD['spells']))
    cases = []
    rng = random.Random(530)

    def add(label, actions, config=None, **extra):
        cases.append(dict(label=label, timers=extra.pop('timers', {}), config=copy.deepcopy(config) if config else {}, actions=actions, **extra))

    def rich_case(label, actions, config=None, world=False, **extra):
        merged = dict(rich_config if world else lean_config)
        merged.update(config or {})
        add(label, actions, merged, timers=copy.deepcopy(TIMERS), registry=REGISTRY, **extra)

    def section(name, save):
        return op('parseSection', name=name, text=J(only(save, name)))

    # New and default profiles, through memory and through real files.
    add('default populate, full state', [op('populate')], dict(full=True))
    add('default file round trip', [op('populate'), op('profileSave'), op('contentHooks', fresh=True), op('profileSave'), op('profileParse'), op('contentHooks'), op('populate'), op('profileSave')])
    add('missing file', [op('profileParse'), op('contentHooks'), op('profileSave'), op('profileParse'), op('populate')], dict(name='New Player'))
    add('hooks before any parse', [op('contentHooks'), op('contentHooks', fresh=True), op('profileSave')])
    add('hooks on a fresh parser with content hooks', [op('contentHooks', fresh=True), op('profileSave')], dict(world=dict(hooks=WORLD['hooks'])))
    add('missing file with content hooks', [op('profileParse'), op('contentHooks'), op('profileSave'), op('profileParse'), op('contentHooks')], dict(world=dict(hooks=WORLD['hooks'])))
    add('no location', [op('populate'), op('saveSection', name='saveCoreData'), op('coreLocation', value=[1, 2, 3]), op('saveSection', name='saveCoreData')], dict(location=False, full=True))
    for name in ['a', 'A b', 'caf\u00e9', '\u65e5\u672c', 'x.y', '%41', 'UPPER', 'trailing ', '\U0001f600']:
        add('player name ' + name, [op('profileParse'), op('contentHooks'), op('profileSave'), op('profileParse'), op('name', value='other'), op('profileParse')], dict(name=name))
    for version in [INT_MIN, -1, 0, 2, 3, 6, 7, INT_MAX]:
        add(f'version {version}', [op('version', value=version), op('saveSection', name='saveVersion'), op('parseSection', name='parseVersion', text=J(dict(version=version))), op('saveSection', name='saveVersion')])

    # Every saver section alone, on a default profile and on a populated one.
    for name in SAVER_SECTIONS:
        add('default ' + name, [op('saveSection', name=name), op('saveSection', name=name)])
        rich_case('populated ' + name, [quiet('parseText', text=rich_text), op('root', text='{"sentinel":7,"version":"old","settings":1}'), op('saveSection', name=name)], world=True)
    rich_case('populated whole save, full state', [op('parseText', text=rich_text), op('populate'), op('process'), op('tick', value=20), op('process'), op('populate')], dict(full=True), world=True)
    rich_case('populated file round trip', [quiet('saveFile', text=rich_text), op('profileParse'), op('contentHooks'), op('profileSave'), op('profileParse'), op('profileSave')], world=True)

    # Every parser section alone: populated input, default input, nothing, and each wrong shape of its key.
    for name, keys in PARSER_SECTIONS.items():
        rich_case('alone ' + name, [section(name, rich), section(name, base), op('parseSection', name=name, text='{}'), op('parseSection', name=name, text=None)])
        for key in keys:
            for wrong in WRONG:
                rich_case(f'{name} {key}={J(wrong)}', [op('parseSection', name=name, text=J(replace(only(rich, name), [key], wrong)))])
            rich_case(f'{name} without {key}', [op('parseSection', name=name, text=J(remove(only(rich, name), [key])))])
    for key in ['totalEXP', 'exp_multiplier']:
        for value in ['7', '10', '9.999', '10.0', '100', 'NaN', 'Infinity', '-0.0', '1e2', '0x1p4', ' 20 ', '20d', '']:
            rich_case(f'parseSkills {key}={value!r}', [op('parseSection', name='parseSkills', text=J(replace(only(rich, 'parseSkills'), [key], value))), op('saveSection', name='saveSkills')])
    for key in ['combatMilestone', 'skillMilestone']:
        for wrong in [None, True, 5, 1.5, 'x', '-3', str(INT_MAX), str(INT_MAX + 1)]:
            rich_case(f'milestone {key}={J(wrong)}', [op('parseSection', name='parseSkills', text=J(replace(only(rich, 'parseSkills'), ['milestone', key], wrong))), op('saveSection', name='saveSkills')])
        rich_case(f'milestone without {key}', [op('parseSection', name='parseSkills', text=J(remove(only(rich, 'parseSkills'), ['milestone', key])))])

    # Whole-file parsing with one top-level key absent or wrong: how far parseData gets.
    for key in sorted(rich):
        rich_case(f'whole without {key}', [op('parseText', text=J(remove(rich, [key]))), op('contentHooks')])
        for wrong in [None, 'x', []]:
            rich_case(f'whole {key}={J(wrong)}', [op('parseText', text=J(replace(rich, [key], wrong)))])
    for key in ['unknown', 'farming', 'Version', 'core_data ', '']:
        rich_case(f'extra key {key!r}', [op('parseText', text=J(replace(rich, [key], {'anything': [1, None, 'x']}))), op('populate')], world=True)

    # Nested keys absent or wrong: the partial state a failing section leaves behind.
    nested = [['core_data', k] for k in sorted(rich['core_data'])] + [['settings', k] for k in sorted(rich['settings'])] + [['bankPinManager', k] for k in sorted(rich['bankPinManager'])]
    nested += [['appearance', 'gender'], ['appearance', 'appearance_cache'], ['appearance', 'appearance_cache', 3], ['appearance', 'appearance_cache', 6, 'color'], ['skills', 0], ['skills', 3, 'lifepoints'],
        ['skills', 5, 'prayerPoints'], ['skills', 23, 'experience'], ['skills', 7, 'id'], ['skills', 7, 'static'], ['skills', 7, 'dynamic'], ['activityData', 'pestPoints'], ['activityData', 'topGrabbed'],
        ['questData', 'draynorLever'], ['questData', 'witchsExperimentKilled'], ['globalData', 'homeTeleportDelay'], ['globalData', 'macroDisabled'], ['globalData', 'assistExperience'],
        ['autocastSpell', 'book'], ['autocastSpell', 'spellId'], ['attributes', 0, 'key'], ['attributes', 0, 'type'], ['attributes', 0, 'value'], ['attributes', 7, 'expiration-time'], ['attributes', 7, 'expirable'],
        ['statistics', 0, 'index'], ['statistics', 0, 'value'], ['configs', 0, 'index'], ['configs', 0, 'value'], ['varps', 0, 'index'], ['varps', 0, 'bitArray'], ['varps', 0, 'bitArray', 1, 'offset'], ['varps', 0, 'bitArray', 1, 'value']]
    for path in nested:
        label = '.'.join(map(str, path))
        name = OWNER[path[0]]
        rich_case(f'nested without {label}', [op('parseSection', name=name, text=J(only(remove(rich, path), name)))])
        for wrong in [None, True, 7, 'x']:
            rich_case(f'nested {label}={J(wrong)}', [op('parseSection', name=name, text=J(only(replace(rich, path, wrong), name)))])
    for path in [['core_data', 'inventory'], ['settings', 'runEnergy'], ['skills', 7, 'static'], ['globalData', 'macroDisabled'], ['bankPinManager', 'longRecovery'], ['appearance', 'appearance_cache', 3]]:
        rich_case(f'whole file without {".".join(map(str, path))}', [op('parseText', text=J(remove(rich, path))), op('contentHooks'), op('populate')])

    # Parser-owned sections: numeric and index boundaries.
    numbers = ['0', '-1', '1', '12', '13', '44', '45', str(INT_MAX), str(INT_MIN), str(INT_MAX + 1), '+3', ' 3', '3 ', '03', '0x3', '3.0', '', '\u0663', 'null']
    for key, name in [('grave_type', 'parseGrave'), ('ironManMode', 'parseIronman'), ('spellbook', 'parseSpellbook'), ('version', 'parseVersion')]:
        for value in numbers + ['2', '3', '4', '192', '193', '430', '431']:
            rich_case(f'{key} {value!r}', [op('parseSection', name=name, text=J({key: value}))])
    for value in [5, -5, 2**31, 2**40, 1.0, 6.5, True, None, [6], {'v': 6}]:
        add(f'version literal {J(value)}', [op('parseSection', name='parseVersion', text=J(dict(version=value))), op('saveSection', name='saveVersion')])
    for value in numbers:
        rich_case(f'emote {value!r}', [op('parseSection', name='parseEmoteManager', text=J(dict(emoteData=['3', value, '40']))), op('saveSection', name='saveEmoteData')])
        rich_case(f'music {value!r}', [op('parseSection', name='parseMusic', text=J(dict(unlockedMusic=['62', value, '333']))), op('saveSection', name='saveMusicPlayer')])
        rich_case(f'autocast book {value!r}', [op('parseSection', name='parseAutocastSpell', text=J(dict(autocastSpell=dict(book=value, spellId='5')))), op('saveSection', name='saveAutocast')])
        rich_case(f'autocast spell {value!r}', [op('parseSection', name='parseAutocastSpell', text=J(dict(autocastSpell=dict(book='0', spellId=value)))), op('saveSection', name='saveAutocast')])
        rich_case(f'config index {value!r}', [op('parseSection', name='parseConfigs', text=J(dict(configs=[dict(index='5', value='6'), dict(index=value, value='8'), dict(index='7', value=value)]))), op('saveSection', name='saveCoreData')])
        rich_case(f'statistic {value!r}', [op('parseSection', name='parseStatistics', text=J(dict(statistics=[dict(index=value, value='8'), dict(index='7', value=value)])))])
        rich_case(f'varp {value!r}', [op('parseSection', name='parseVarps', text=J(dict(varps=[dict(index='5', bitArray=[dict(offset='1', value='1')]), dict(index=value, bitArray=[dict(offset='31', value='3'), dict(offset='32', value='5')]),
            dict(index='9', bitArray=[dict(offset=value, value='1')]), dict(index='10', bitArray=[dict(offset='2', value=value)])]))), op('saveSection', name='saveCoreData')])
    for books in [['0', '5'], ['1', '3'], ['2', '9'], ['2', '10'], ['3', '5'], ['-1', '5'], ['0', '-1']]:
        rich_case(f'autocast lookup {books}', [op('parseSection', name='parseAutocastSpell', text=J(dict(autocastSpell=dict(book=books[0], spellId=books[1])))), op('saveSection', name='saveAutocast')])
    for wrong in WRONG:
        for key, name in [('emoteData', 'parseEmoteManager'), ('unlockedMusic', 'parseMusic'), ('statistics', 'parseStatistics'), ('configs', 'parseConfigs'), ('varps', 'parseVarps')]:
            rich_case(f'{key} element {J(wrong)}', [op('parseSection', name=name, text=J({key: [wrong]}))])
        rich_case(f'varp bits {J(wrong)}', [op('parseSection', name='parseVarps', text=J(dict(varps=[dict(index='4', bitArray=wrong)])))])
        rich_case(f'varp bit {J(wrong)}', [op('parseSection', name='parseVarps', text=J(dict(varps=[dict(index='3', bitArray=[dict(offset='1', value='1')]), dict(index='4', bitArray=[dict(offset='0', value='1'), wrong])]))), op('saveSection', name='saveCoreData')])
    add('achievements absent resets rewards', [op('parseSection', name='parseAchievements', text='{}'), op('parseSection', name='parseAchievements', text='{"achievementDiaries":[{"KARAMJA":{}}]}')])
    add('legacy configs then save', [op('parseSection', name='parseConfigs', text='{"configs":[{"index":"10","value":"5"},{"index":"1048","value":"bad"},{"index":"11","value":"0"}]}'), op('saveSection', name='saveCoreData'), op('coreVarp', index=10, value=6, save=True), op('saveSection', name='saveCoreData')])

    # Unicode and escapes in every place a string is carried.
    for index, text in enumerate(STRINGS):
        attributes = [dict(type='str', key='k' + str(index), value=text), dict(type='str', key=text, value='v'), dict(type=text, key='typed', value='v')]
        add(f'string {index} parsed', [op('parseSection', name='parseAttributes', text=J(dict(attributes=attributes))), op('saveSection', name='saveAttributes'), op('contentHooks', fresh=True), op('profileSave'), op('profileParse'), op('saveSection', name='saveAttributes')])
        add(f'string {index} set', [op('attr', key='/save:s', kind='str', value=text), op('attr', key='/save:' + text, kind='int', value=index), op('pin', pin=text), op('saveSection', name='saveAttributes'), op('saveSection', name='saveBankPinData'),
            op('parseSection', name='parseBankPin', text=J(dict(bankPinManager=dict(pin=text, longRecovery=False))))])
        add(f'string {index} in content', [op('populate'), op('contentHooks', fresh=True), op('profileSave'), op('profileParse'), op('contentHooks')], dict(world=dict(hooks=[[text, J(text)], ['hook', J({text: [text]})]], pouches=[[text, J([text])]], poh=J({text: text}))))
    escapes = ['"\\u0041\\u00e9\\u65E5"', '"\\ud83d\\ude00"', '"\\/\\b\\f\\n\\r\\t\\"\\\\"', '"\\x41"', '"\\u00zz"', '"\\u12"', '"a\\qb"', '"\\"', '"tab\tliteral"', '"line\nliteral"', '"\\u0000"', '"\\uFFFF"', '"\\u2028"']
    for index, literal in enumerate(escapes):
        add(f'escape {index}', [op('parseSection', name='parseAttributes', text='{"attributes":[{"type":"str","key":"e","value":' + literal + '}]}'), op('saveSection', name='saveAttributes')])

    # Attribute types, expirations and the legacy attribute file.
    kinds = [('int', 0), ('int', INT_MIN), ('int', INT_MAX), ('long', LONG_MIN), ('long', LONG_MAX), ('short', -32768), ('short', 32767), ('byte', -128), ('byte', 127), ('byte', 0), ('bool', True), ('bool', False),
        ('str', ''), ('location', [3222, 3218, 0]), ('location', [-1, 16384, 7]), ('double', '1.5'), ('double', 'NaN'), ('null', None)]
    for kind, value in kinds:
        add(f'attribute {kind} {value}', [op('attr', key='/save:a', kind=kind, value=value), op('attr', key='plain', kind=kind, value=value), op('saveSection', name='saveAttributes'), op('attr', key='/save:b', kind=kind, value=value, expires=5000),
            op('saveSection', name='saveAttributes'), op('populate')], dict(clock=100))
    add('attribute saved list only', [op('attrSaved', key='ghost'), op('saveSection', name='saveAttributes'), op('attr', key='ghost', kind='int', value=1), op('attrSaved', key='ghost'), op('saveSection', name='saveAttributes'),
        op('attr', key='/save:z', kind='int', value=2, expires=None), op('saveSection', name='saveAttributes'), op('populate')])
    for expiry, clock, step in [('999', 1000, 0), ('1000', 1000, 0), ('1001', 1000, 0), ('1000', 999, 1), ('1002', 1000, 1), (str(LONG_MAX), LONG_MAX, 0), (str(LONG_MIN), LONG_MIN, 1), ('bad', 0, 0), ('', 0, 0), ('1.0', 0, 0), (None, 0, 0), (5, 3, 0)]:
        attributes = [{'type': 'int', 'key': 'a', 'value': '1', 'expirable': True, 'expiration-time': expiry}, {'type': 'int', 'key': 'b', 'value': '2', 'expirable': True, 'expiration-time': expiry},
            {'type': 'int', 'key': 'c', 'value': '3', 'expirable': True, 'expiration-time': expiry}, {'type': 'bool', 'key': 'd', 'value': True}]
        add(f'expiry {expiry} at {clock}+{step}', [op('parseSection', name='parseAttributes', text=J(dict(attributes=attributes))), op('saveSection', name='saveAttributes')], dict(clock=clock, clockStep=step))
    for expirable in [None, False, 0, 'true', [], {}]:
        add(f'expirable {J(expirable)}', [op('parseSection', name='parseAttributes', text=J(dict(attributes=[{'type': 'int', 'key': 'first', 'value': '1'}, {'type': 'int', 'key': 'a', 'value': '1', 'expirable': expirable, 'expiration-time': '5'}])))], dict(clock=1))
    values = ['5', 5, -5, 5.0, True, None, '', 'x', '32768', '-32769', str(INT_MAX + 1), str(LONG_MAX), str(LONG_MAX + 1), '/w==', 'QQ==', 'QQ', 'Q', '====', 'QUJD', '[1, 2, 3]', '1,2,3', '[1,2]', '[a, b, c]', [1, 2, 3], {'x': 1}]
    for kind in ['int', 'long', 'short', 'str', 'bool', 'byte', 'location', 'unknown', None, 5]:
        for value in values:
            add(f'attribute parse {J(kind)} {J(value)}', [op('parseSection', name='parseAttributes', text=J(dict(attributes=[dict(type='int', key='before', value='1'), dict(type=kind, key='k', value=value), dict(type='int', key='after', value='2')]))), op('saveSection', name='saveAttributes')])
    add('attribute duplicate keys', [op('parseSection', name='parseAttributes', text=J(dict(attributes=[dict(type='int', key='k', value='1'), dict(type='str', key='k', value='two'), dict(type='int', key='k', value='3')]))), op('saveSection', name='saveAttributes')])
    xml = '<?xml version="1.0" encoding="UTF-8"?><attributes><GameAttribute key="legacy" type="int" value="42"/><GameAttribute key="flag" type="bool" value="true"/><GameAttribute key="name" type="string" value="caf\u00e9"/></attributes>'
    add('legacy attribute file', [quiet('attributeFile', text=xml), op('parseText', text=base_text), op('saveSection', name='saveAttributes'), op('parseSection', name='parseAttributes', text='{"attributes":[]}'), quiet('attributeFile', delete=True),
        op('parseSection', name='parseAttributes', text='{}')])
    add('legacy attribute file missing', [op('parseSection', name='parseAttributes', text='{}'), op('saveSection', name='saveAttributes')])
    add('legacy attribute file malformed', [quiet('attributeFile', text='<attributes><GameAttribute key="a" type="int" value="1"/><GameAttribute key="b" type="int" value="x"/>'), op('parseSection', name='parseAttributes', text='{}'),
        quiet('attributeFile', text='<attributes><GameAttribute key="a" type="int" value="1"/><GameAttribute key="b" type="int" value="x"/><GameAttribute key="c" type="int" value="3"/></attributes>'), op('parseSection', name='parseAttributes', text='{}')])
    add('attributes with a null entry', [op('parseSection', name='parseAttributes', text='{"attributes":[{"type":"int","key":"a","value":"1"},null,{"type":"int","key":"b","value":"2"}]}')])

    # Skills: legacy versions, the experience squish and its permadeath exemption.
    for version in [0, 2, 3, 6]:
        for drop in [[], ['lifepoints'], ['prayerPoints'], ['lifepoints', 'prayerPoints']]:
            save = copy.deepcopy(rich)
            for key in drop:
                del save['skills'][3 if key == 'lifepoints' else 5][key]
            rich_case(f'skills version {version} without {drop}', [quiet('version', value=version), section('parseSkills', save), op('saveSection', name='saveSkills')])
    for multiplier in ['9.999', '10', '10.0', '20.0', '25', '1.0E3', 'NaN', 'Infinity', '-10', '0']:
        for permadeath in [False, True]:
            save = replace(rich, ['exp_multiplier'], multiplier)
            for index, skill in enumerate(save['skills']):
                skill['experience'] = str([0, 83, 1154, 13034431, 200000000, 2e9, 1e300, 5e-324][index % 8] * 4)
            actions = [quiet('attr', key='permadeath', kind='bool', value=False)] if permadeath else []
            rich_case(f'squish {multiplier} permadeath={permadeath}', actions + [section('parseSkills', save), op('saveSection', name='saveSkills')])
    rich_case('squish with null permadeath attribute', [op('attr', key='permadeath', kind='null', value=None), section('parseSkills', replace(rich, ['exp_multiplier'], '20.0'))])
    rich_case('squish in a whole save', [op('parseText', text=J(replace(rich, ['exp_multiplier'], '20.0'))), op('populate')])
    rich_case('squish exempt in a whole save', [op('parseText', text=J(replace(replace(rich, ['exp_multiplier'], '20.0'), ['attributes'], [dict(type='bool', key='permadeath', value=True)]))), op('saveSection', name='saveSkills')])
    rows = [dict(id='3', static='5', dynamic='6', experience='7.0', lifepoints='8'), dict(id='5', static='9', dynamic='10', experience='11.0', prayerPoints='12.5')]
    for skills in [[], rows, rows * 14, [rows[1], rows[0]], [dict(id='24', static='1', dynamic='1', experience='0')], [dict(id='-1', static='1', dynamic='1', experience='0')], [None], [[]], ['x'],
            [dict(id='0', static='x', dynamic='1', experience='0')], [dict(id='0', static='1', dynamic='1', experience='x')], [dict(id=0, static=1, dynamic=1, experience=0)], [dict(id='0', static='1', dynamic='1', experience='0')] * 4 + rows]:
        for version in [2, 6]:
            add(f'skill rows {J(skills)[:60]} v{version}', [quiet('version', value=version), op('parseSection', name='parseSkills', text=J(dict(skills=skills, totalEXP='1.0', exp_multiplier='1.0'))), op('saveSection', name='saveSkills')])
    extremes = dict(experience=['0.0', '-0.0', 'NaN', 'Infinity', '-Infinity', '1.0E-5', '0.001', '1.0E7', '9999999.0', '4.9E-324', '1.7976931348623157E308', '0.1', '100.0', '2.0E8', '1.23456789E8', '13034431.0'] + ['0.5'] * 8,
        staticLevels=[INT_MIN, -1, 0, 1, 99, 100, INT_MAX] + [5] * 17, dynamicLevels=[INT_MAX, 0, -1, INT_MIN] + [7] * 20, prayerPoints='-0.0', lifepoints=INT_MIN, experienceGained='1.0E21', experienceMultiplier='NaN')
    add('skill extremes', [op('skills', fields=extremes), op('populate'), op('skills', fields=dict(combatMilestone=0, skillMilestone=1)), op('saveSection', name='saveSkills'), op('skills', fields=dict(combatMilestone=INT_MIN, skillMilestone=0)),
        op('saveSection', name='saveSkills'), op('skills', fields=dict(combatMilestone=1, skillMilestone=INT_MIN)), op('saveSection', name='saveSkills'), op('skills', fields=dict(combatMilestone=0, skillMilestone=0)), op('saveSection', name='saveSkills')])

    # Settings, PIN, ironman, appearance and spellbook boundaries through direct state.
    for value in [INT_MIN, -1, 0, 1, 255, INT_MAX]:
        fields = dict(brightness=value, musicVolume=value, soundEffectVolume=value, areaSoundVolume=value, publicChatSetting=value, privateChatSetting=value, clanChatSetting=value, tradeSetting=value, assistSetting=value, specialEnergy=value, attackStyleIndex=value)
        add(f'settings ints {value}', [op('settings', fields=fields), op('retaliate', value=value % 2 == 0), op('saveSection', name='saveSettings')])
    for energy in ['0.0', '-0.0', '100.0', '99.99999999999999', '1.0E-4', '0.001', '9999999.0', '1.0E7', 'NaN', 'Infinity', '-Infinity', '37.5', '0.30000000000000004']:
        add(f'run energy {energy}', [op('settings', fields=dict(runEnergy=energy, singleMouseButton=True, acceptAid=True, specialToggled=True, weight='12.5')), op('saveSection', name='saveSettings')])
    for energy in ['0', '-0.0', '100', '1e2', '1E-4', '0x1p3', 'NaN', '-Infinity', ' 5 ', '5f', '5D', '', 'x', '1,5']:
        add(f'run energy parsed {energy!r}', [op('parseSection', name='parseSettings', text=J(replace(only(base, 'parseSettings'), ['settings', 'runEnergy'], energy))), op('saveSection', name='saveSettings')])
    for pending, tried, clock, step in [(-1, 0, 0, 0), (-1, 1, 0, 0), (0, 0, 0, 0), (1, 1, 0, 0), (1, 1, 1, 0), (2, 2, 1, 0), (5, 5, 4, 1), (5, 6, 4, 1), (-2, -2, -3, 0), (LONG_MAX, LONG_MAX, LONG_MAX - 1, 0), (LONG_MAX, LONG_MAX, LONG_MAX, 0),
            (LONG_MIN, LONG_MIN, LONG_MIN, 0), (10, 10, LONG_MAX, 1), (-1, 5, 4, 5)]:
        for status in [0, 1, 2]:
            add(f'pin delays {pending},{tried} at {clock}+{step} status {status}', [op('pin', pin='0000', status=status, longRecovery=status == 1, pendingDelay=pending, tryDelay=tried), op('saveSection', name='saveBankPinData')], dict(clock=clock, clockStep=step))
    add('pin null status', [op('pin', status=None), op('saveSection', name='saveBankPinData'), op('populate')])
    add('pin none', [op('pin', pin=None, longRecovery=True), op('saveSection', name='saveBankPinData')])
    for value in ['', '0000', '12345', 'abcd', '-1', 'null', '\u0661\u0662\u0663\u0664']:
        for status in ['0', '1', '2', '3', '-1', 'x', None]:
            add(f'pin parse {value!r} {status!r}', [op('parseSection', name='parseBankPin', text=J(dict(bankPinManager=dict(pin=value, status=status, longRecovery=False, pendingDelay='7', tryDelay='8')))), op('saveSection', name='saveBankPinData')], dict(clock=5))
    for delay in ['-1', '0', str(LONG_MAX), str(LONG_MIN), str(LONG_MAX + 1), '1.0', 'x', '', None, 5, True]:
        add(f'pin delay {J(delay)}', [op('parseSection', name='parseBankPin', text=J(dict(bankPinManager=dict(longRecovery=True, pendingDelay=delay)))), op('parseSection', name='parseBankPin', text=J(dict(bankPinManager=dict(longRecovery=False, tryDelay=delay)))),
            op('saveSection', name='saveBankPinData')], dict(clock=-5))
    for mode in [0, 1, 2, 3, None]:
        add(f'ironman {mode}', [op('ironman', mode=mode), op('saveSection', name='saveIronManData'), op('populate')])
        add(f'accept aid as ironman {mode}', [op('settings', fields=dict(acceptAid=True)), op('saveSection', name='saveSettings'), op('ironman', mode=mode), op('saveSection', name='saveSettings'), op('settings', fields=dict(acceptAid=False)),
            op('saveSection', name='saveSettings')])
    for index in [0, 3, 6]:
        add(f'quest item {index} null', [op('questItem', index=index, null=True), op('saveSection', name='saveSavedData'), op('populate'), op('questItem', index=index, id=7, amount=INT_MIN), op('saveSection', name='saveSavedData')])
    add('appearance null gender', [op('gender', value=None), op('saveSection', name='saveAppearance'), op('populate'), section('parseAppearance', base)])
    for index in [0, 3, 6]:
        add(f'appearance null part {index}', [op('part', index=index, null=True), op('saveSection', name='saveAppearance'), op('populate'), section('parseAppearance', base), op('part', index=index, look=INT_MIN, color=INT_MAX), op('saveSection', name='saveAppearance')])
    cache = base['appearance']['appearance_cache']
    for gender in ['0', '1', '2', '-1', '127', '128', '-128', '-129', 'x', '', '0.0', 1, None, True]:
        add(f'appearance gender {J(gender)}', [op('parseSection', name='parseAppearance', text=J(dict(appearance=dict(gender=gender, appearance_cache=cache)))), op('saveSection', name='saveAppearance')])
    for cached in [[], cache[:6], cache + [dict(look='9', color='9')], [None] * 7, cache[:3] + [None] + cache[4:], cache[:3] + [dict(look='x', color='1')] + cache[4:], cache[:3] + [dict(look='1')] + cache[4:], cache[:6] + [[]], 'x', None, {}]:
        add(f'appearance cache {J(cached)[:50]}', [op('parseSection', name='parseAppearance', text=J(dict(appearance=dict(gender='1', appearance_cache=cached)))), op('saveSection', name='saveAppearance')])
    for value in [INT_MIN, -1, 0, 192, 193, 430, 431, INT_MAX]:
        add(f'spellbook state {value}', [op('spellbook', value=value), op('saveSection', name='saveSpellbook')])

    # Core state reached directly: raw items, varps with each save flag, tabs and the bank selector.
    add('core raw state', [op('coreRaw', slot=0, id=65537, amount=-9, charge=65536, index=-1), op('coreRaw', container='bank', slot=495, id=2, index=17), op('coreRaw', container='equipment', slot=13, id=7, amount=INT_MAX, charge=0),
        op('coreRaw', container='secondary', slot=1, id=3), op('coreTab', index=10, value=INT_MIN), op('coreTab', index=0, value=INT_MAX, secondary=True), op('coreSecondary', value=True), op('populate'), op('parseText', text=base_text), op('populate')])
    for keys in [[i * 64 for i in range(40)], list(reversed([i * 64 for i in range(40)])), [INT_MIN, -1, 0, 1, INT_MAX], list(range(13)), [0, 16, 32, 48, 1, 17]]:
        add(f'varp order {keys[:4]}', [quiet('coreVarp', index=k, value=i + 1, save=rng.choice([None, False, True, True])) for i, k in enumerate(keys)] + [op('saveSection', name='saveCoreData')])
    for location in ['1', '1,2', '1,2,3', '1,2,3,4', '1,2,3,bad', '1,2,3,', ' 1,2,3', '-16385,16384,-5', '2147483647,-2147483648,-2147483648', '\u0967\u0968,\u0969,\u096a', '']:
        add(f'location {location!r}', [op('parseSection', name='parseCore', text=J(replace(only(base, 'parseCore'), ['core_data', 'location'], location))), op('saveSection', name='saveCoreData')])
    literals = ['1e999', '-1e999', '1E400', '0.0', '-0', '1e2', '100000000000000000000', '9223372036854775807', '9223372036854775808', '-9223372036854775808', '-9223372036854775809', '1.', '.5', '01', '+1', '0x10', 'NaN', 'Infinity', '1e', '1e+']
    for literal in literals:
        for key, name in [('version', 'parseVersion'), ('totalEXP', 'parseSkills'), ('spellbook', 'parseSpellbook'), ('grave_type', 'parseGrave')]:
            add(f'number literal {literal} at {key}', [op('parseSection', name=name, text=J(replace(only(base, name), [key], 'LITERAL')).replace('"LITERAL"', literal))])
        add(f'number literal {literal} in inventory', [op('parseSection', name='parseCore', text=J(replace(only(base, 'parseCore'), ['core_data', 'inventory'], [dict(slot='0', id='1', amount='LITERAL', charge='1000')])).replace('"LITERAL"', literal))])
        add(f'number literal {literal} in skills', [op('parseSection', name='parseSkills', text=J(replace(only(base, 'parseSkills'), ['skills', 2, 'experience'], 'LITERAL')).replace('"LITERAL"', literal)), op('saveSection', name='saveSkills')])
        add(f'number literal {literal} in attributes', [op('parseSection', name='parseAttributes', text=J(dict(attributes=[dict(type='str', key='k', value='LITERAL'), dict(type='long', key='n', value='LITERAL')])).replace('"LITERAL"', literal)), op('saveSection', name='saveAttributes')])
        add(f'number literal {literal} in saved data', [op('parseSection', name='parseSavedData', text=J(replace(only(base, 'parseSavedData'), ['activityData', 'pestPoints'], 'LITERAL')).replace('"LITERAL"', literal)), op('saveSection', name='saveActivityData')])
        add(f'number literal {literal} in a whole file', [quiet('saveFile', text=J(replace(base, ['version'], 'LITERAL')).replace('"LITERAL"', literal)), op('profileParse')])

    # Files: truncation, bytes that are not UTF-8, surrounding text and JSON-simple's lax grammar.
    cuts = sorted(set(list(range(0, 24)) + [len(rich_text) * i // 40 for i in range(1, 40)] + [len(rich_text) - i for i in range(1, 16)]))
    for cut in cuts:
        rich_case(f'truncated at {cut}', [quiet('saveFile', text=rich_text[:cut]), op('profileParse'), op('contentHooks'), op('profileSave')])
    documents = ['', ' ', '\n', '{}', '{ }', '[]', 'null', 'true', '5', '"x"', '{}{}', '{} x', '{}\n\n', '\ufeff{}', '{"a":1,}', '{,"a":1}', '{"a" 1}', '{"a"::1}', '{"a":1 "b":2}', '{"a":1,,"b":2}', '{a:1}', "{'a':1}", '{"a":[1 2]}', '{"a":[,]}',
        '{"a":[1,]}', '{"a":}', '{"a"}', '{"a":1', '{"a":[1}', '{"a":{"b":1]}', '{"version":"1","version":"2"}', '{"version":"2","version":7}', '// c\n{}', '/* c */{}', '{"a":tru}', '{"a":nul}', '{"a":TRUE}', '{"a":-}', '{"a":1e5}', '{"a":"\\u00e9"}',
        '{"a":"unterminated}', '{"a":"\t"}', '[{}]', ' {"version":"4"} ', '\t\r\n{"version":"4"}\t\r\n', '{"version":"4"}\x00', '\x00{"version":"4"}', '{"version" : "4" , "x" : [ ] }']
    for index, document in enumerate(documents):
        add(f'document {index} {document[:24]!r}', [quiet('saveFile', text=document), op('profileParse'), op('contentHooks'), op('profileSave')])
        add(f'document text {index} {document[:24]!r}', [op('parseText', text=document)])
    wrapped = ['{PREFIX}', '{PREFIX} ', ' {PREFIX}', '{PREFIX}\n', '{PREFIX}}', '{PREFIX}x', '{PREFIX}{}', '\ufeff{PREFIX}', '[{PREFIX}]']
    for index, wrapper in enumerate(wrapped):
        add(f'surrounded {index}', [quiet('saveFile', text=wrapper.replace('{PREFIX}', base_text)), op('profileParse'), op('contentHooks'), op('profileSave')])
    byte_strings = ['ff', 'c3', 'c328', 'e282', 'e28228', 'f09f98', 'eda080', 'c080', 'f8888080', '80', 'e0a0', 'f4908080', 'c3a9', 'f09f9880', 'efbfbd', 'e0', 'f0', 'c3c3a9', 'e282e282ac', 'fe', 'f09f', 'eda0bdedb880']
    # parseData reaches the attributes right after the core section, so a small carrier is enough to see decoded text.
    carrier = '{"core_data":{"inventory":[],"bank":[],"equipment":[],"location":"1,2,3"},"attributes":[{"type":"str","key":"k","value":"'.encode().hex()
    for index, hexed in enumerate(byte_strings):
        add(f'bytes {index} {hexed}', [quiet('saveFile', hex=carrier + hexed + '"}]}'.encode().hex()), op('profileParse'), op('saveSection', name='saveAttributes')])
    for hexed in ['ff', 'eda080', 'f09f9880', 'c3a9']:
        add(f'bytes {hexed} in a whole save', [quiet('saveFile', hex=base_text[:-1].encode().hex() + '2c' + carrier[carrier.index('2261747472'):] + hexed + '"}]}'.encode().hex()), op('profileParse'), op('contentHooks'), op('profileSave'), op('profileParse'),
            op('saveSection', name='saveAttributes')])
    tails = ['', '80', 'bf', '9f', 'a0', '90', '8f', '28', '8080', 'a080', '9fbf', '8028', 'bfbfbf', '908080', '8fbfbf', '808028', '80bf8080', 'a0bdedb880', 'b080']
    for lead in ['c0', 'c1', 'c2', 'df', 'e0', 'e1', 'ec', 'ed', 'ee', 'ef', 'f0', 'f1', 'f3', 'f4', 'f5', 'f7', 'f8', 'fb', 'fc', 'fd', 'fe', 'ff', '80', 'bf']:
        add(f'byte lead {lead}', [quiet('saveFile', hex=carrier + '7c'.join(lead + tail for tail in tails) + '"}]}'.encode().hex()), op('profileParse')])
    for index, hexed in enumerate(['7b7d', 'efbbbf7b7d', 'fffe7b007d00', '7bff7d', '7b2261ff223a317d', 'c3', '7b7dc3', 'ff7b7d', '7b7dff', '7bc37d']):
        add(f'byte document {index} {hexed}', [quiet('saveFile', hex=hexed), op('profileParse'), op('contentHooks'), op('profileSave')])
    add('file replaced between loads', [quiet('saveFile', text=rich_text), op('profileParse'), quiet('saveFile', text=base_text), op('profileParse'), quiet('saveFile', delete=True), op('profileParse'), op('contentHooks'), op('profileSave')],
        dict(lean_config), timers=copy.deepcopy(TIMERS), registry=REGISTRY)
    add('save file path is a directory', [quiet('saveFile', directory=True), op('profileParse'), op('contentHooks', fresh=True), op('profileSave'), op('profileParse')])
    add('save directory is a file', [quiet('blockSaveDirectory'), op('profileParse'), op('contentHooks'), op('profileSave'), op('profileParse'), op('profileSave')])
    add('save requires saveParsed', [op('profileSave'), op('saveParsed', value=True), op('profileSave'), op('saveParsed', value=False), op('attr', key='/save:changed', kind='int', value=1), op('profileSave'), op('profileParse')])

    # The unported systems' sections: values the saver reads, and what the parser hands over.
    add('world empty', [op('populate')], dict(world=dict(quests=dict(points=0, list=[]), emotes=[], diaries=[], pets=[], familiar=None, pouches=[], hooks=[], house=dict(location=0, style=0, crest=0, servant=None, rooms=[]))))
    for points in [INT_MIN, -1, 0, 1, INT_MAX]:
        add(f'quest points {points}', [op('saveSection', name='saveQuests')], dict(world=dict(quests=dict(points=points, list=[[points, points], [0, 0], [16, INT_MAX], [32, INT_MIN], [48, 1], [64, 2]]))))
    add('quest order', [op('saveSection', name='saveQuests'), op('world', world=dict(quests=dict(points=1, list=[[i * 16, i] for i in range(14)]))), op('saveSection', name='saveQuests'), op('world', world=dict(quests=dict(points=2, list=[[3, 99], [1000, 1], [-7, 2]]))),
        op('saveSection', name='saveQuests')], dict(world=dict(quests=dict(points=0, list=[[i, i] for i in reversed(range(20))]))))
    for emotes in [[], [0], list(range(22)), [22], [21, 21, 21], [44, 0, 22, 44], list(reversed(range(45)))]:
        add(f'emotes {emotes[:5]}', [op('saveSection', name='saveEmoteData'), op('parseSection', name='parseEmoteManager', text='{"emoteData":["0","44","22","44"]}'), op('saveSection', name='saveEmoteData')], dict(world=dict(emotes=emotes)))
    for unlocked in [[], [62], [151, 76, 2, 62, 333, 7, -5], [-5, 7, 333, 62, 2, 76, 151]]:
        add(f'music {unlocked}', [op('saveSection', name='saveMusicPlayer'), op('parseSection', name='parseMusic', text='{"unlockedMusic":["333","62","333"]}'), op('saveSection', name='saveMusicPlayer')], dict(world=dict(songs=WORLD['songs'], unlocked=unlocked)))
    add('music many', [op('saveSection', name='saveMusicPlayer')], dict(world=dict(songs=[[i, (i * 7) % 300] for i in range(300)], unlocked=[(i * 11) % 300 for i in range(300)])))
    for autocast in [None, [0, 0], [1, INT_MAX], [2, INT_MIN], [0, -1]]:
        add(f'autocast {autocast}', [op('saveSection', name='saveAutocast'), op('world', world=dict(autocast=None)), op('saveSection', name='saveAutocast')], dict(world=dict(autocast=autocast)))
    familiars = [None, dict(pet=1555), dict(pet=INT_MIN), dict(originalId=1, ticks=2, specialPoints=3, lifepoints=4), dict(originalId=INT_MIN, ticks=INT_MAX, specialPoints=-1, lifepoints=INT_MIN),
        dict(originalId=1, ticks=2, specialPoints=3, lifepoints=0, burden=[]), dict(originalId=1, ticks=2, specialPoints=3, lifepoints=INT_MAX, burden=[[29, 0, 0, 0]]), dict(originalId=1, ticks=2, specialPoints=3, lifepoints=5, burden=[[i, i % 8, i + 1, 1000 - i] for i in range(30)])]
    for index, familiar in enumerate(familiars):
        add(f'familiar {index}', [op('saveSection', name='saveFamiliarManager')], dict(world=dict(familiar=familiar, pets=WORLD['pets'])))
    add('pet order', [op('saveSection', name='saveFamiliarManager')], dict(world=dict(pets=[[i * 16, [[str(i), str(i / 4)]]] for i in range(14)] + [[5, []], [-3, [['1', '2']]], [0, [['9', '9']]]])))
    houses = [dict(location=0, style=0, crest=0, servant=None, rooms=[]), dict(location=INT_MIN, style=INT_MAX, crest=-1, servant=dict(type=0, uses=0, item=None, greet=False), rooms=[]),
        dict(location=1, style=1, crest=1, servant=dict(type=INT_MAX, uses=INT_MIN, item=[0, 0], greet=True), rooms=[dict(z=z, x=x, y=y, properties=z * 100 + x * 10 + y, rotation=(x + y) % 4, hotspots=[x - 1, y - 1, -1, z]) for z in range(4) for x in range(0, 8, 3) for y in range(0, 8, 7)]),
        dict(location=5, style=5, crest=5, servant=dict(type=1, uses=1, item=[7, INT_MAX], greet=False), rooms=[dict(z=3, x=7, y=7, properties=1, rotation=1, hotspots=[-1] * 6), dict(z=0, x=0, y=0, properties=2, rotation=2, hotspots=list(range(-2, 9)))])]
    for index, house in enumerate(houses):
        add(f'house {index}', [op('saveSection', name='saveHouseData')], dict(world=dict(house=house)))
    for index, diaries in enumerate([[], WORLD['diaries'], [dict(type=t, started=[t % 2 == 0] * 3, completed=[[t % 3 == 0] * (t + 1)] * 3, rewarded=[t % 2 == 1] * 3) for t in range(6)], [dict(type=1, started=[True], completed=[[]], rewarded=[])] * 3]):
        add(f'diaries {index}', [op('saveSection', name='saveAchievementData')], dict(world=dict(diaries=diaries)))
    for index, pouches in enumerate([[], [['pouches', '[]']], [['pouches', 'null']], [['pouches', '[1]'], ['pouches', '[2]']], [['version', '"overwritten"'], ['core_data', '5'], ['statistics', '{}'], ['zz', '1.5']]]):
        add(f'pouches {index}', [op('saveSection', name='savePouches'), op('populate'), op('contentHooks', fresh=True), op('profileSave'), op('profileParse')], dict(world=dict(pouches=pouches)))
    for index, poh in enumerate(['{}', '{"containers":{}}', '{"containers":{"a":{"b":[1,2.5,"3",null,false]}}}', '{"x":"\\u00e9\\n"}']):
        add(f'poh {index}', [op('saveSection', name='savePOHStorageRoom'), op('parseSection', name='parsePOHStorable', text=J(dict(pohstorage=json.loads(poh))))], dict(world=dict(poh=poh)))
    for index, hooks in enumerate([[], [['a', '1']], [['a', '1'], ['a', '2']], [['version', '"hooked"'], ['pohstorage', 'null'], ['pouches', '{"k":[]}']], [[f'key{i}', J(i)] for i in range(40)]]):
        add(f'hooks {index}', [op('populate'), op('contentHooks', fresh=True), op('profileSave'), op('profileParse'), op('contentHooks')], dict(world=dict(hooks=hooks)))
    handed = dict(quests=[{}, {'points': '5', 'questStages': [{'questId': '1', 'questStage': '2'}], 'extra': None}], familiarManager=[{}, {'petDetails': {'12': [{'hunger': '1.0', 'growth': '2.0'}]}, 'currentPet': '12'}],
        houseData=[{}, {'location': '1', 'rooms': [{'z': '0', 'hotspots': []}]}], achievementDiaries=[[], [{'KARAMJA': {'startedLevels': [True]}}, None, 5]], pouches=[[], [{'id': '5509'}, None]], pohstorage=[{}, {'containers': {'x': {}}}])
    for key, values in handed.items():
        for index, value in enumerate(values):
            add(f'handed {key} {index}', [op('parseSection', name=OWNER[key], text=J({key: value}))])
    add('handed object key order', [op('parseSection', name='parseQuests', text='{"quests":{"zz":1,"a":2,"points":"3","m":{"y":1,"x":2},"Aa":1,"BB":2,"n":1.0,"e":1e3,"big":123456789012,"s":"\\u00e9/"}}')])

    # Failures raised by the unported systems and by timer callbacks, at each point they can occur.
    squished = replace(rich, ['exp_multiplier'], '20.0')
    for tag in ['quests.parse', 'grave', 'familiar.parse', 'house.parse', 'diary.parse', 'pouches.parse', 'poh.parse', 'packet', 'summon', 'combat']:
        rich_case(f'parse fails at {tag}', [op('parseText', text=J(squished)), op('contentHooks'), op('saveSection', name='saveSkills')], dict(fail={tag: 1}))
    rich_case('parse fails at diary.reset', [op('parseText', text=J(remove(rich, ['achievementDiaries'])))], dict(fail={'diary.reset': 1}))
    for tag in ['pouches.save', 'poh.save', 'hook.save']:
        rich_case(f'save fails at {tag}', [quiet('parseText', text=rich_text), op('populate'), op('contentHooks'), op('profileSave'), op('populate')], dict(fail={tag: 1}), world=True)
    rich_case('second hook fails', [quiet('parseText', text=rich_text), op('populate'), op('contentHooks'), op('contentHooks')], dict(fail={'hook.save': 2, 'hook.parse': 2}), world=True)
    rich_case('failure armed later', [quiet('parseText', text=rich_text), op('saveSection', name='savePOHStorageRoom'), op('failAt', tag='poh.save', at=2), op('saveSection', name='savePOHStorageRoom'), op('populate'), op('saveSection', name='savePOHStorageRoom')])
    for stage in ['save', 'parse', 'before', 'register']:
        for callback in [op('fail'), op('coreVarp', index=32, value=99, save=True), op('coreLocation', value=[1, 2, 3]), op('coreSecondary', value=False)]:
            label = 'created0' if stage in ['before', 'register', 'parse'] else 'p'
            rich_case(f'timer {stage} runs {callback["op"]}', [op('parseText', text=rich_text), op('register', timer='p'), op('process'), op('saveSection', name='saveCoreData'), op('contentHooks'), op('profileSave'), op('profileParse')],
                scripts=[dict(timer=label, event=stage, actions=[callback])])

    # Random profiles: loaded by the original, saved, loaded again from the file and saved again.
    for index in range(24):
        save = copy.deepcopy(rich)
        core = save['core_data']
        for key, size in [('inventory', 28), ('bank', 496), ('bankSecondary', 496), ('equipment', 14)]:
            core[key] = [row(slot, rng.randrange(8), rng.choice([0, 1, 5, INT_MAX, rng.randrange(1, 10**6)]), rng.choice([0, 1000, 65535])) for slot in sorted(rng.sample(range(size), rng.randrange(0, 9)))]
        core['bankTabs'] = [dict(index=str(i), startSlot=str(rng.randrange(-5, 500))) for i in range(11)]
        core['bankTabsSecondary'] = [dict(index=str(rng.randrange(11)), startSlot=str(rng.randrange(-5, 500))) for _ in range(4)]
        core['useSecondaryBank'] = rng.random() < 0.5
        core['location'] = f'{rng.randrange(0, 16384)},{rng.randrange(0, 16384)},{rng.randrange(4)}'
        core['varp'] = [dict(index=str(rng.randrange(-50, 2000)), value=str(rng.randrange(INT_MIN, INT_MAX))) for _ in range(rng.randrange(0, 30))]
        core['timers'] = {'persist': {'ticksLeft': str(rng.randrange(-100, 100))}} if rng.random() < 0.7 else {}
        for skill in save['skills']:
            skill.update(static=str(rng.randrange(1, 100)), dynamic=str(rng.randrange(0, 120)), experience=repr(rng.choice([0.0, rng.random() * 2e8, float(rng.randrange(0, 200000000)), rng.random()])))
        save['skills'][3]['lifepoints'] = str(rng.randrange(0, 120))
        save['skills'][5]['prayerPoints'] = repr(rng.choice([0.0, 1.0, rng.random() * 99]))
        save['totalEXP'] = repr(rng.random() * 1e9)
        save['exp_multiplier'] = rng.choice(['1.0', '5.0', '2.5'])
        if rng.random() < 0.5:
            del save['milestone']
        else:
            save['milestone'] = dict(combatMilestone=str(rng.randrange(0, 5)), skillMilestone=str(rng.randrange(0, 5)))
        for key in ['brightness', 'musicVolume', 'soundEffectVolume', 'areaSoundVolume', 'publicChatSetting', 'privateChatSetting', 'clanChatSetting', 'tradeSetting', 'assistSetting', 'specialEnergy', 'attackStyle']:
            save['settings'][key] = str(rng.randrange(-3, 120))
        save['settings']['runEnergy'] = repr(rng.choice([100.0, 0.0, rng.random() * 100]))
        for key in ['singleMouse', 'disableChatEffects', 'splitPrivate', 'acceptAid', 'runToggled', 'retaliation']:
            save['settings'][key] = rng.random() < 0.5
        save['attributes'] = [dict(type='int', key=f'k{i}', value=str(rng.randrange(INT_MIN, INT_MAX))) for i in range(rng.randrange(0, 6))] + [dict(type='str', key=rng.choice(STRINGS[:12]) + str(i), value=rng.choice(STRINGS)) for i in range(rng.randrange(0, 4))]
        save['appearance'] = dict(gender=str(rng.randrange(2)), appearance_cache=[dict(look=str(rng.randrange(0, 300)), color=str(rng.randrange(0, 30))) for _ in range(7)])
        save['spellbook'] = rng.choice(['192', '193', '430'])
        save['bankPinManager'] = dict(longRecovery=rng.random() < 0.5, pendingDelay=str(rng.randrange(0, 4000)), tryDelay=str(rng.randrange(0, 4000)))
        if rng.random() < 0.6:
            save['bankPinManager'].update(pin=''.join(rng.choice('0123456789') for _ in range(4)), status=str(rng.randrange(3)))
        if rng.random() < 0.5:
            del save['ironManMode']
        else:
            save['ironManMode'] = str(rng.randrange(4))
        save['emoteData'] = [str(rng.randrange(45)) for _ in range(rng.randrange(0, 8))]
        for name in ['activityData', 'questData', 'globalData']:
            for key, value in save[name].items():
                if isinstance(value, bool):
                    save[name][key] = rng.random() < 0.5
                elif isinstance(value, str):
                    save[name][key] = str(rng.randrange(0, 120))
                elif isinstance(value, list) and value and isinstance(value[0], bool):
                    save[name][key] = [rng.random() < 0.5 for _ in value]
        save['globalData']['assistExperience'] = [repr(rng.choice([0.0, rng.random() * 30000])) for _ in range(9)]
        config = dict(clock=2000, world=dict(WORLD, hooks=[]))
        add(f'random {index}', [op('parseText', text=J(save)), op('contentHooks'), op('profileSave'), op('profileParse'), op('populate')], config, timers=copy.deepcopy(TIMERS), registry=REGISTRY)
    return cases


def second_generation(first):
    """Feed texts the original wrote back to it in a profile that starts empty."""
    cases = []
    seen = set()
    for source in first:
        for action, state in zip(source['actions'], source['expected']):
            result = state.get('result')
            text = result.get('text') if isinstance(result, dict) else None
            if action['op'] != 'populate' or not text or text in seen or len(seen) >= 40:
                continue
            seen.add(text)
            config = copy.deepcopy(source['config'])
            config.pop('fail', None)
            cases.append(dict(label='reload of ' + source['label'], timers=copy.deepcopy(source['timers']), registry=source.get('registry', {}), config=config,
                actions=[dict(op='parseText', text=text), op('populate')], reload=text))
    return cases
