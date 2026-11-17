"""The engine timer corpus: designed cases per timer, their interplay, persistence and random sequences.

`cases()` yields (case, sweep) pairs; `sweep` names the operation whose recorded calls the generator fails one at a
time in a second pass (None for no sweep). Entities are players unless `kind` says npc or familiar; the case's first
entity owns the timers unless an operation names another (`e`).
"""
import random

CLASSES = ['AntiMacro', 'Disease', 'DragonFireImmunity', 'Frozen', 'FrozenImmunity', 'HealOverTime', 'Miasmic',
           'MiasmicImmunity', 'Poison', 'PoisonImmunity', 'SkillRestore', 'Skulled', 'SpellbookSwap', 'Teleblock']
IDS = ['antimacro', 'disease', 'dragonfire:immunity', 'frozen', 'frozen:immunity', 'healovertime', 'miasmic',
       'miasmic:immunity', 'poison', 'poison:immunity', 'skillrestore', 'skulled', 'spellbook:swap', 'teleblock']
NEEDED_EVENTS = ['manual_hit', 'skill_packet', 'perk', 'equipment', 'varp', 'varbit', 'message', 'audio', 'debug', 'skull_icon',
                 'set_skulled', 'reset_skull', 'hook', 'unhook', 'dispatch', 'register_logout', 'clear_logout', 'select_auto_spell',
                 'open_tab', 'extension', 'autocast', 'get_attribute', 'set_attribute', 'remove_attribute', 'poison_immune',
                 'zone_restricted', 'dialogue_ids', 'skill_random', 'non_skill_random', 'create_event_npc', 'spawn_location',
                 'init_npc', 'terminate_npc', 'roll_loot', 'next_int', 'ticks', 'modal', 'delay', 'name', 'location', 'log',
                 'exception', 'artificial', 'rights', 'uid', 'player_by_uid', 'npc_task', 'familiar_owner', 'remove_weak_scripts',
                 'foreign_hook']

MAX, MIN = 2 ** 31 - 1, -2 ** 31


def draws(seed, bounds):
    """java.util.Random(seed).nextInt(bound) for each bound, to pick seeds that reach a boundary."""
    mask = (1 << 48) - 1
    state = (seed ^ 0x5DEECE66D) & mask
    out = []
    for bound in bounds:
        def bits31():
            nonlocal state
            state = (state * 0x5DEECE66D + 0xB) & mask
            return state >> 17
        r = bits31()
        m = bound - 1
        if bound & m == 0:
            out.append((bound * r) >> 31)
            continue
        u = r
        r = u % bound
        while u - r + m >= 2 ** 31:
            u = bits31()
            r = u % bound
        out.append(r)
    return out


def op(operation, /, **kw):
    return dict(op=operation, **kw)


def player(**kw):
    return dict(kind='player', name=kw.pop('name', 'Zezima'), location=kw.pop('location', [3222, 3218, 0]), uid=kw.pop('uid', 77), **kw)


def npc(**kw):
    return dict(kind='npc', name=kw.pop('name', 'Goblin'), location=kw.pop('location', [3250, 3240, 0]), **kw)


def familiar(**kw):
    return dict(kind='familiar', name=kw.pop('name', 'Spirit wolf'), location=kw.pop('location', [3221, 3218, 0]), **kw)


def ent(index):
    return {'entity': index}


GROUPS = []


def case(group, entities, ops, sweep=None, **kw):
    GROUPS.append(group)
    return dict(group=group, entities=entities, ops=ops, **kw), sweep


def runs(count, n):
    return [op('run', n=n) for _ in range(count)]


def arguments():
    """Every class's getTimer/retrieveInstance over argument shapes."""
    shapes = [None, [], [5], [0], [-1], [MAX], [True], ['x'], [{'long': 5}], [5, True], [5, False], [5, 'x'], [5, 7, 3],
              [ent(1)], [ent(1), 30], [ent(1), {'long': 3}], [ent(1), 30, 9], [ent(0), 5]]
    for cls in CLASSES:
        for shape in shapes:
            ops = [op('spawn', **{'class': cls}, args=shape or [], label='s')]
            ops += [op('register', timer='s'), op('process'), op('save'), op('process'), op('save'), op('has', **{'class': cls})]
            yield case('arguments', [player(), npc()], ops, sweep=1 if shape in (None, [5]) else None)
    for ident in IDS + ['POISON', 'Frozen:Immunity', 'missing', 'Teleblock']:
        for shape in [[], [7], [ent(1), 12]]:
            ops = [op('spawnId', id=ident, args=shape, label='s'), op('getOrStartId', id=ident, args=shape), op('hasId', id=ident.lower()),
                   op('process'), op('save')]
            yield case('identifiers', [player(), npc()], ops)


def poison():
    for severity in [-1, 0, 1, 4, 5, 6, 10, 30, MAX, MIN]:
        for victim in [player(), npc(), familiar(owner=1)]:
            ops = [op('start', **{'class': 'Poison'}, args=[ent(1), severity], label='p'), op('process')] + runs(6, 30) + [op('save')]
            yield case('poison', [victim, player(name='Source', uid=12, static={'18': 50}, dynamic={'18': 50})], ops,
                       sweep=1 if severity == 30 and victim['kind'] == 'player' else None)
    # Slayer requirements on an NPC victim, from a player and from a familiar.
    for task in [None, 40, 50, 60]:
        for source in ['player', 'familiar', 'familiar-without-owner', 'npc', 'self']:
            entities = [npc(task=task), player(name='Slayer', uid=5, static={'18': 50}, dynamic={'18': 50})]
            if source == 'player':
                index = 1
            elif source == 'npc':
                entities.append(npc(name='Spider'))
                index = 2
            elif source == 'self':
                index = 0
            else:
                entities.append(familiar(owner=1 if source == 'familiar' else None))
                index = 2
            ops = [op('start', **{'class': 'Poison'}, args=[ent(index), 18], label='p'), op('process'), op('run', n=30), op('save')]
            yield case('poison-slayer', entities, ops, sweep=0 if task == 60 and source in ('player', 'familiar') else None)
    # applyPoison over immunity and an existing poison.
    for existing in [None, 10, 20, 30]:
        for immune in [False, True]:
            for immunity in [False, True]:
                ops = []
                if immunity:
                    ops.append(op('start', **{'class': 'PoisonImmunity'}, args=[50], label='i'))
                if existing is not None:
                    ops.append(op('start', **{'class': 'Poison'}, args=[ent(1), existing], label='p'))
                ops += [op('process'), op('applyPoison', source=2, severity=20), op('isPoisoned'), op('process'), op('run', n=30),
                        op('applyPoison', source=1, severity=25), op('curePoison'), op('isPoisoned'), op('curePoison'), op('process')]
                yield case('apply-poison', [player(immune=immune), player(name='A', uid=3), npc(name='Scorpion')], ops,
                           sweep=len(ops) - 4 if existing == 20 and not immune and not immunity else None)
    # Uninitialized sources: getTimer with too few or wrong arguments.
    for args in [[True], [ent(1)], ['x', 5], [ent(1), 'x']]:
        ops = [op('spawn', **{'class': 'Poison'}, args=args, label='p'), op('register', timer='p'), op('process'), op('run', n=30),
               op('save'), op('timerSave', timer='p'), op('field', timer='p', field='source', value=1), op('timerSave', timer='p')]
        yield case('poison-uninitialized', [player(), npc()], ops)
    # Persistence.
    texts = ['{"poison":{"source-uid":12,"severity":"20"}}', '{"poison":{"source-uid":"12","severity":"20"}}',
             '{"poison":{"source-uid":99,"severity":"7"}}', '{"poison":{"source-uid":-1,"severity":"7"}}',
             '{"poison":{"source-uid":"x","severity":"7"}}', '{"poison":{"source-uid":12}}', '{"poison":{"severity":"7"}}',
             '{"poison":{"source-uid":12,"severity":7}}', '{"poison":{"source-uid":12,"severity":"0"}}',
             '{"poison":{"source-uid":12.5,"severity":"3"}}', '{"poison":{"source-uid":2147483648,"severity":"3"}}']
    for text in texts:
        ops = [op('parse', text=text), op('save'), op('process'), op('run', n=30), op('save')]
        yield case('poison-parse', [player(), player(name='Source', uid=12)], ops, uids={'12': 1}, sweep=0 if text == texts[0] else None)


def disease():
    for seed in [1, 7, 42, 530, 2009]:
        for hits in [None, 1, 3, 0, -1]:
            for victim in [player(), npc()]:
                args = [] if hits is None else [hits]
                ops = [op('start', **{'class': 'Disease'}, args=args, label='d'), op('process')] + runs(4, 30)
                if hits is None and seed == 7:
                    ops = [op('start', **{'class': 'Disease'}, args=args, label='d'), op('process')] + runs(26, 30)
                ops.append(op('save'))
                yield case('disease', [victim], ops, seed=seed, sweep=2 if hits == 3 and seed == 42 and victim['kind'] == 'player' else None)
    for second in [[], [5]]:
        ops = [op('start', **{'class': 'Disease'}, args=[2], label='d'), op('process'), op('start', **{'class': 'Disease'}, args=second, label='e'),
               op('process'), op('run', n=30), op('save'), op('death'), op('process')]
        yield case('disease-again', [player(dynamic={'0': 3, '1': 1, '2': 50})], ops, seed=3)
    for text in ['{"disease":{"hitsLeft":"4"}}', '{"disease":{"hitsLeft":4}}', '{"disease":{}}', '{"disease":{"hitsLeft":"-2"}}']:
        yield case('disease-parse', [player()], [op('parse', text=text), op('process'), op('run', n=30), op('save')], seed=11)


def restore():
    levels = [
        dict(static={'0': 50, '1': 40, '2': 60, '3': 50, '4': 30, '5': 43, '6': 70, '23': 20}, dynamic={'0': 55, '1': 38, '2': 60, '3': 50, '4': 1, '5': 10, '6': 99, '23': 5}, lp=30),
        dict(static={'3': 99}, dynamic={'3': 99}, lp=1),
        dict(static={'3': 10}, dynamic={'3': 10}, lp=10),
        dict(static={'3': 10, '7': 5}, dynamic={'3': 10, '7': 0}, lp=12),
    ]
    for i, lv in enumerate(levels):
        for kind in ['player', 'npc']:
            for extras in [{}, {'worn': {'9': 11133}}, {'sponge': True, 'worn': {'9': 1}}, {'special': 45}]:
                spec = (player if kind == 'player' else npc)(**lv, **extras)
                ops = [op('start', **{'class': 'SkillRestore'}, label='r'), op('process')] + runs(5, 25) + [op('run', n=50)]
                yield case('restore', [spec], ops, sweep=2 if i == 0 and kind == 'player' and not extras else None)
    for special in [0, 45, 95, 100, 120]:
        ops = [op('start', **{'class': 'SkillRestore'}, label='r'), op('process'), op('run', n=50), op('run', n=1), op('run', n=50)]
        yield case('restore-special', [player(special=special)], ops)
    prayers = ['RAPID_HEAL', 'RAPID_RESTORE', 'THICK_SKIN', 'PROTECT_ITEMS']
    for prayer in prayers:
        for present in [False, True]:
            ops = [op('start', **{'class': 'SkillRestore'}, label='r'), op('process')] if present else []
            ops += [op('dispatch', event={'type': 'PrayerActivated', 'prayer': prayer}), op('process'), op('run', n=60),
                    op('dispatch', event={'type': 'PrayerDeactivated', 'prayer': prayer}), op('run', n=60),
                    op('getOrStart', **{'class': 'SkillRestore'})]
            if not present:
                ops = [op('start', **{'class': 'Teleblock'}, args=[500], label='hold'), op('process')] + ops
            yield case('restore-prayer', [player(**levels[0])], ops, sweep=len(ops) - 4 if prayer == 'RAPID_RESTORE' and present else None)
    # Automatic installation, double registration (hooks are not duplicated) and wrapping counters.
    for kind in ['player', 'npc']:
        spec = player(**levels[0]) if kind == 'player' else npc(**levels[0])
        ops = [op('auto'), op('process'), op('auto'), op('start', **{'class': 'SkillRestore'}, label='second'), op('process'),
               op('dispatch', event={'type': 'PrayerActivated', 'prayer': 'RAPID_HEAL'}), op('run', n=10),
               op('fieldOf', **{'class': 'SkillRestore'}, field='since', value=[0, MAX]), op('fieldOf', **{'class': 'SkillRestore'}, field='restore', value=[0, MIN]),
               op('run', n=3), op('fieldOf', **{'class': 'SkillRestore'}, field='since', value=[24, MAX]), op('run', n=2), op('death'), op('process')]
        yield case('restore-auto', [spec], ops, seed=17, sweep=0 if kind == 'player' else None)


def freezing():
    for args in [[], [5], [5, True], [5, False], [0, True], [10, 'x'], [-3, True], [1, True]]:
        for victim in [player(), npc()]:
            ops = [op('start', **{'class': 'Frozen'}, args=args, label='f'), op('process'), op('save'), op('run', n=6),
                   op('has', **{'class': 'FrozenImmunity'}), op('start', **{'class': 'Frozen'}, args=[4, True], label='g'), op('process'),
                   op('run', n=8), op('save'), op('start', **{'class': 'Frozen'}, args=[3], label='h'), op('process'), op('save')]
            yield case('frozen', [victim], ops, sweep=3 if args == [5, True] and victim['kind'] == 'player' else None)
    for args in [[], [3], [0], [-1]]:
        ops = [op('start', **{'class': 'Frozen'}, args=[9], label='f'), op('process'), op('start', **{'class': 'FrozenImmunity'}, args=args, label='i'),
               op('process'), op('run', n=4), op('save'), op('death'), op('process')]
        yield case('frozen-immunity', [player()], ops)
    for text in ['{"frozen":{"ticksLeft":"5","applyImmunity":true}}', '{"frozen":{"ticksLeft":"5","applyImmunity":false}}',
                 '{"frozen":{"ticksLeft":"5","applyImmunity":"true"}}', '{"frozen":{"ticksLeft":"5"}}', '{"frozen":{"applyImmunity":true}}',
                 '{"frozen":{"ticksLeft":5,"applyImmunity":null}}', '{"frozen:immunity":{"ticksLeft":"6"}}',
                 '{"frozen:immunity":{"ticksLeft":"6"},"frozen":{"ticksLeft":"3","applyImmunity":true}}']:
        yield case('frozen-parse', [player()], [op('parse', text=text), op('save'), op('process'), op('run', n=8), op('save')])


def miasmic():
    for args in [[], [3], [0], [ent(0), 7]]:
        for victim in [player(), npc()]:
            ops = [op('start', **{'class': 'Miasmic'}, args=args, label='m'), op('process'), op('save'), op('run', n=4), op('save'),
                   op('start', **{'class': 'Miasmic'}, args=[2], label='n'), op('process'), op('start', **{'class': 'HealOverTime'}, args=[1, 10, 3], label='h'),
                   op('process'), op('run', n=110), op('save')]
            yield case('miasmic', [victim], ops, sweep=3 if args == [3] and victim['kind'] == 'player' else None)
    for args in [[], [1, 10, 3], [2, 7, 2], [1, 5, 10], [1, 0, 2], [1, 9, 0], [1, -4, 2], [1, MAX, MAX]]:
        for lp in [1, 9, 10]:
            ops = [op('start', **{'class': 'HealOverTime'}, args=args, label='h'), op('process'), op('save'), op('run', n=6), op('save'), op('run', n=100)]
            yield case('heal', [player(lp=lp), npc()], ops, sweep=3 if args == [1, 10, 3] and lp == 1 else None)
    for pre in ['Miasmic', 'MiasmicImmunity']:
        ops = [op('start', **{'class': pre}, args=[20], label='m'), op('process'), op('start', **{'class': 'HealOverTime'}, label='h'),
               op('process'), op('start', **{'class': 'MiasmicImmunity'}, args=[2], label='i'), op('process'), op('run', n=3), op('save')]
        yield case('heal-miasmic', [player(lp=3)], ops)
    for text in ['{"healovertime":{"ticksLeft":"1","healRemaining":"6","healPerTick":"2"}}', '{"healovertime":{"ticksLeft":"1","healRemaining":"6"}}',
                 '{"miasmic":{"ticksLeft":"2"},"miasmic:immunity":{"ticksLeft":"3"}}', '{"miasmic:immunity":{"ticksLeft":"3"},"miasmic":{"ticksLeft":"2"}}']:
        yield case('miasmic-parse', [player(lp=2)], [op('parse', text=text), op('save'), op('process'), op('run', n=5), op('save')])


def simple():
    for cls, args in [('Teleblock', []), ('Teleblock', [3]), ('Teleblock', [0]), ('Skulled', []), ('Skulled', [2]), ('Skulled', [MAX])]:
        for victim in [player(), npc()]:
            ops = [op('start', **{'class': cls}, args=args, label='t'), op('process'), op('save'), op('hasId', id=cls.lower()), op('run', n=4),
                   op('save'), op('start', **{'class': cls}, args=args, label='u'), op('process'), op('death'), op('process'),
                   op('start', **{'class': cls}, args=[50], label='v'), op('remove', timer='v'), op('process')]
            yield case('simple', [victim], ops, sweep=0 if args == [] and victim['kind'] == 'player' else None)
    for cls in ['DragonFireImmunity', 'PoisonImmunity']:
        for args in [[], [51], [1], [0], [-1], [52]]:
            for victim in [player(), npc()]:
                ops = [op('start', **{'class': cls}, args=args, label='t'), op('process'), op('run', n=2), op('save'),
                       op('set', fields={'modal': True}), op('run', n=2), op('set', fields={'modal': False, 'delay': 1000}), op('run', n=2),
                       op('set', fields={'delay': 0}), op('run', n=3), op('save')]
                yield case('immunity', [victim, npc()], ops, sweep=2 if args == [51] and victim['kind'] == 'player' else None)
        for text in ['{"%s":{"ticksRemaining":"3"}}', '{"%s":{"ticksRemaining":51}}', '{"%s":{}}', '{"%s":{"ticksRemaining":"x"}}']:
            ident = 'dragonfire:immunity' if cls == 'DragonFireImmunity' else 'poison:immunity'
            yield case('immunity-parse', [player()], [op('parse', text=text % ident), op('save'), op('process'), op('run', n=4), op('save')])
    # Soft and normal timers under a modal and a script delay.
    for blocker in [{'modal': True}, {'delay': 50}, {}]:
        for victim in [player(**blocker), npc(**blocker)]:
            ops = [op('start', **{'class': 'Poison'}, args=[ent(0), 6], label='p'), op('start', **{'class': 'DragonFireImmunity'}, args=[40], label='d'),
                   op('start', **{'class': 'Teleblock'}, args=[2], label='t'), op('process'), op('run', n=31), op('save')]
            yield case('soft', [victim], ops)


def swap():
    events = [
        {'type': 'SpellCast', 'book': 'LUNAR', 'spell': 12},
        {'type': 'SpellCast', 'book': 'LUNAR', 'spell': 13},
        {'type': 'SpellCast', 'book': 'MODERN', 'spell': 12},
        {'type': 'SpellbookChange', 'old': 'LUNAR', 'new': 'MODERN', 'source': 'SPELLBOOK_SWAP_CAST'},
        {'type': 'SpellbookChange', 'old': 'MODERN', 'new': 'LUNAR', 'source': 'SPELLBOOK_SWAP_RESTORE'},
        {'type': 'SpellbookChange', 'old': 'MODERN', 'new': 'ANCIENT', 'source': 'ALTAR'},
        {'type': 'SpellbookChange', 'old': 'LUNAR', 'new': 'MODERN', 'source': 'MAGIC_CAPE_PERK'},
        {'type': 'DialogueClose', 'dialogue': 'swap'},
        {'type': 'DialogueClose', 'dialogue': 'other'},
        {'type': 'DialogueClose', 'dialogue': 'none'},
        {'type': 'PrayerActivated', 'prayer': 'RAPID_HEAL'},
    ]
    dialogues = {'swap': [1, 3264731], 'other': [5, 6], 'none': None}
    for book in [192, 193, 430, 0]:
        for weapon, autocast, select in [(None, False, None), (90, True, True), (90, False, False), (None, True, True), (90, True, None)]:
            spec = player(book=book, autocast=autocast, attrs={} if select is None else {'autocast_select': select})
            if weapon is not None:
                spec['weapon'] = weapon
            for event in events:
                ops = [op('start', **{'class': 'SpellbookSwap'}, label='s'), op('process'), op('dispatch', event=event), op('process'),
                       op('save'), op('logout'), op('run', n=2)]
                yield case('swap-events', [spec], ops, dialogues=dialogues,
                           sweep=2 if book == 192 and weapon == 90 and autocast and select and event['type'] == 'SpellCast' and event['book'] == 'MODERN' else None)
            ops = [op('start', **{'class': 'SpellbookSwap'}, label='s'), op('process'), op('logout'), op('save'), op('run', n=199), op('run', n=1),
                   op('logout'), op('death'), op('process')]
            yield case('swap-expiry', [spec], ops, dialogues=dialogues, sweep=5 if book == 193 and weapon == 90 and autocast and select else None)
    # Foreign hooks around the timer's own, two swaps, NPC owners and persistence.
    for before in [False, True]:
        ops = ([op('hookForeign', event='SpellCast', name='quest')] if before else []) + [
            op('start', **{'class': 'SpellbookSwap'}, label='s'), op('process'), op('hookForeign', event='SpellbookChange', name='tracker'),
            op('start', **{'class': 'SpellbookSwap'}, label='t'), op('process'), op('dispatch', event={'type': 'SpellCast', 'book': 'MODERN', 'spell': 1}),
            op('process'), op('dispatch', event={'type': 'SpellbookChange', 'old': 'MODERN', 'new': 'LUNAR', 'source': 'ALTAR'}), op('process')]
        yield case('swap-hooks', [player(book=192, weapon=90, autocast=True)], ops)
    yield case('swap-npc', [npc()], [op('start', **{'class': 'SpellbookSwap'}, label='s'), op('process'),
                                     op('dispatch', event={'type': 'SpellCast', 'book': 'MODERN', 'spell': 1}), op('process'), op('run', n=200)])
    for text in ['{"spellbook:swap":{"ticksLeft":"3"}}', '{"spellbook:swap":{"ticksLeft":"0"}}', '{"spellbook:swap":{"ticksLeft":"-5"}}']:
        yield case('swap-parse', [player(book=193)], [op('parse', text=text), op('process'), op('run', n=4), op('save'), op('logout')])


def antimacro():
    for kind, extra in [('player', {}), ('player', {'artificial': True}), ('player', {'rights': 'ADMINISTRATOR'}), ('player', {'rights': 'PLAYER_MODERATOR'}),
                        ('npc', {}), ('familiar', {})]:
        spec = (player if kind == 'player' else npc if kind == 'npc' else familiar)(**extra)
        ops = [op('start', **{'class': 'AntiMacro'}, label='a'), op('process'), op('save'), op('auto'), op('process'), op('save')]
        yield case('antimacro-register', [spec], ops, seed=5, sweep=0 if not extra and kind == 'player' else None)
    scenarios = [
        ('plain', {}, []),
        ('restricted', {'restricted': True}, []),
        ('existing', {'attrs': {'re-npc': True}}, []),
        ('paused', {}, [op('pause')]),
        ('unpaused', {'rights': 'ADMINISTRATOR'}, [op('unpause')]),
        ('forced', {}, [op('forceEvent', event='MAZE')]),
        ('forced-null', {}, [op('forceEvent')]),
        ('skill-recent', {'lastTrained': 10, 'lastXpGain': 990}, []),
        ('skill-old', {'lastTrained': 10, 'lastXpGain': 700}, []),
        ('skill-none', {'lastTrained': 3, 'lastXpGain': 999}, []),
        ('skill-boundary', {'lastTrained': 10, 'lastXpGain': 751}, []),
        ('skill-boundary2', {'lastTrained': 10, 'lastXpGain': 750}, []),
        ('missing-spawn', {}, []),
    ]
    for seed in [1, 2, 3, 4, 99]:
        for name, fields, before in scenarios:
            ops = [op('start', **{'class': 'AntiMacro'}, label='a'), op('process'), *before, op('field', timer='a', field='next', value=1000),
                   op('tick', value=1000), op('process'), op('eventNpc'), op('loot'), op('terminate'), op('save'),
                   op('field', timer='a', field='next', value=1000), op('process'), op('save')]
            yield case('antimacro-run', [player(**fields)], ops, seed=seed, tick=10, skillRandom={'10': 'EVIL_BOB', '3': None},
                       nonSkill=['CERTER', 'GENIE', 'SURPRISE_EXAM'], spawnMissing=[0] if name == 'missing-spawn' else [],
                       loot={'CERTER': [[995, 80], [1623, 1]]}, sweep=4 + len(before) if seed == 1 and name in ('plain', 'skill-recent', 'forced') else None)
    # The skill-event roll at its boundary: seeds whose third draw (after the two interval draws) is 64, 65 or 66.
    for wanted in [64, 65, 66]:
        seed = next(s for s in range(100000) if draws(s, [6001, 6001, 100])[2] == wanted)
        for fields in [{'lastTrained': 10, 'lastXpGain': 990}, {'lastTrained': 3, 'lastXpGain': 990}, {'lastTrained': 10, 'lastXpGain': 750},
                       {'lastTrained': 10, 'lastXpGain': 751}]:
            ops = [op('start', **{'class': 'AntiMacro'}, label='a'), op('process'), op('field', timer='a', field='next', value=1000),
                   op('tick', value=1000), op('process'), op('save')]
            yield case('antimacro-roll', [player(**fields)], ops, seed=seed, tick=10, skillRandom={'10': 'EVIL_BOB'},
                       nonSkill=['GENIE'])
    for text in ['{"antimacro":{"ticksRemaining":"500"}}', '{"antimacro":{"ticksRemaining":500}}', '{"antimacro":{"ticksRemaining":"x"}}',
                 '{"antimacro":{}}', '{"antimacro":{"ticksRemaining":"+5"}}', '{"antimacro":{"ticksRemaining":"-3"}}',
                 '{"antimacro":{"ticksRemaining":"2147483648"}}', '{"antimacro":{"ticksRemaining":"0"}}', '{"antimacro":{"ticksRemaining":true}}',
                 '{"antimacro":{"ticksRemaining":2.5}}', '{"antimacro":{"ticksRemaining":null}}', '{"antimacro":{"ticksRemaining":"١٢"}}']:
        yield case('antimacro-parse', [player()], [op('parse', text=text), op('save'), op('process'), op('save')], seed=8, tick=40)
    for name in ['pause', 'unpause', 'forceEvent', 'eventNpc', 'terminate', 'loot']:
        yield case('antimacro-companion', [player(attrs={'re-npc': None})], [op(name), op('start', **{'class': 'AntiMacro'}, label='a'), op(name), op('process'), op(name)], seed=6)
    for text, command in [('revent', None), ('revent -p zezima', None), ('revent -p Mod Mark -e drill demon', None), ('-e maze -p x', None),
                          ('revent  -p  a  b ', None), ('revent -e -p', None), ('foo -p bar', 'foo'), ('revent -e sandwich lady', None),
                          ('revent -e certer -e genie', None), ('REVENT -p ÄÖ', None), ('revent -p\tx', None), ('revent -e evil_bob', None),
                          ('', None), ('-p', None), ('revent -p Strasse -e QUIZ master', 'revent')]:
        o = op('parseArgs', text=text)
        if command is not None:
            o['command'] = command
        yield case('antimacro-args', [player()], [o])


def persistence():
    starts = [('Poison', [ent(1), 12]), ('Disease', [3]), ('Frozen', [8, True]), ('FrozenImmunity', [4]), ('HealOverTime', [2, 9, 3]),
              ('Miasmic', [6]), ('MiasmicImmunity', [9]), ('PoisonImmunity', [30]), ('DragonFireImmunity', [60]), ('Skulled', [20]),
              ('SpellbookSwap', []), ('Teleblock', [15]), ('AntiMacro', []), ('SkillRestore', [])]
    ops = []
    for cls, args in starts:
        ops.append(op('start', **{'class': cls}, args=args, label=cls))
    ops += [op('save'), op('process'), op('save'), op('run', n=3), op('save'), op('death'), op('save'), op('process'), op('save'), op('clear'), op('save')]
    yield case('persistence', [player(book=193), player(name='Source', uid=12)], ops, seed=21, tick=100, uids={'12': 1})
    yield case('persistence-npc', [npc(), player(name='Source', uid=12)], ops, seed=21, tick=100)
    # The section the persistence case writes after its first process, loaded into a fresh player.
    written = ('{"dragonfire:immunity":{"ticksRemaining":"60"},"miasmic:immunity":{"ticksLeft":"9"},"disease":{"hitsLeft":"3"},'
               '"spellbook:swap":{"ticksLeft":"200"},"antimacro":{"ticksRemaining":"6546"},"teleblock":{"ticksLeft":"15"},'
               '"healovertime":{"healRemaining":"9","ticksLeft":"2","healPerTick":"3"},"poison:immunity":{"ticksRemaining":"30"},'
               '"skulled":{"ticksLeft":"20"},"frozen:immunity":{"ticksLeft":"4"},"poison":{"source-uid":12,"severity":"12"}}')
    yield case('persistence-load', [player(book=193), player(name='Source', uid=12)],
               [op('parse', text=written), op('process'), op('save'), op('run', n=5), op('save')], seed=21, tick=100, uids={'12': 1})
    texts = ['{"teleblock":{"ticksLeft":"4"},"skulled":{"ticksLeft":"2"},"unknown":{"ticksLeft":"1"},"skillrestore":{}}',
             '{"TELEBLOCK":{"ticksLeft":"4"}}', '{"teleblock":{"ticksLeft":"4"},"skulled":[]}', '{"skulled":{"ticksLeft":"x"},"teleblock":{"ticksLeft":"4"}}',
             '{"teleblock":{"ticksLeft":"2147483647"}}', '{"teleblock":{"ticksLeft":"-2147483648"}}']
    for text in texts:
        yield case('persistence-parse', [player()], [op('parse', text=text), op('save'), op('process'), op('save'), op('run', n=3), op('save')], tick=MAX - 1)
    for cls, args in starts:
        if cls in ('SkillRestore',):
            continue
        for text in ['{}', '{"ticksLeft":"9","ticksRemaining":"9","hitsLeft":"9","severity":"9","source-uid":12,"healRemaining":"9","healPerTick":"9","applyImmunity":true}']:
            ops = [op('spawn', **{'class': cls}, args=args, label='t'), op('timerParse', timer='t', text=text), op('timerSave', timer='t'),
                   op('register', timer='t'), op('process'), op('timerSave', timer='t')]
            yield case('persistence-members', [player(), player(name='Source', uid=12)], ops, uids={'12': 1}, seed=4, tick=10)


def sequences():
    rng = random.Random(530)
    pool = ['start', 'process', 'run', 'save', 'death', 'removeClass', 'dispatch', 'applyPoison', 'curePoison', 'has', 'remove', 'parse', 'set', 'auto', 'logout']
    starts = [('Poison', lambda: [ent(rng.choice([0, 1, 2])), rng.choice([1, 6, 18, 30])]), ('Disease', lambda: [rng.choice([1, 2, 25])]),
              ('Frozen', lambda: [rng.choice([1, 3, 10]), rng.choice([True, False])]), ('FrozenImmunity', lambda: [rng.choice([1, 7])]),
              ('HealOverTime', lambda: [1, rng.choice([5, 10]), rng.choice([1, 3])]), ('Miasmic', lambda: [rng.choice([1, 4])]),
              ('MiasmicImmunity', lambda: [rng.choice([1, 4])]), ('PoisonImmunity', lambda: [rng.choice([2, 51])]),
              ('DragonFireImmunity', lambda: [rng.choice([2, 51])]), ('Skulled', lambda: [rng.choice([1, 5])]), ('SpellbookSwap', lambda: []),
              ('Teleblock', lambda: [rng.choice([1, 5])]), ('SkillRestore', lambda: []), ('AntiMacro', lambda: [])]
    events = [{'type': 'PrayerActivated', 'prayer': 'RAPID_HEAL'}, {'type': 'PrayerDeactivated', 'prayer': 'RAPID_RESTORE'},
              {'type': 'SpellCast', 'book': 'MODERN', 'spell': 3}, {'type': 'SpellbookChange', 'old': 'MODERN', 'new': 'LUNAR', 'source': 'ALTAR'},
              {'type': 'DialogueClose', 'dialogue': 'swap'}]
    for number in range(160):
        kind = rng.choice(['player', 'player', 'npc'])
        owner = player(book=rng.choice([192, 430]), weapon=90, autocast=rng.choice([True, False]), lp=rng.choice([3, 10]),
                       dynamic={'0': rng.choice([1, 5]), '3': 10}, static={'0': 3}) if kind == 'player' else npc(lp=4, task=rng.choice([None, 1, 99]))
        ops, labels = [], []
        for step in range(rng.randrange(8, 20)):
            name = rng.choice(pool)
            if name == 'start':
                cls, make = rng.choice(starts)
                label = f'x{step}'
                labels.append(label)
                ops.append(op('start', **{'class': cls}, args=make(), label=label))
            elif name == 'run':
                ops.append(op('run', n=rng.choice([1, 5, 30])))
            elif name == 'removeClass' or name == 'has':
                ops.append(op(name, **{'class': rng.choice(CLASSES)}))
            elif name == 'remove':
                if labels:
                    ops.append(op('remove', timer=rng.choice(labels)))
            elif name == 'dispatch':
                ops.append(op('dispatch', event=rng.choice(events)))
            elif name == 'applyPoison':
                ops.append(op('applyPoison', source=rng.choice([0, 1, 2]), severity=rng.choice([5, 20, 40])))
            elif name == 'parse':
                ops.append(op('parse', text=rng.choice(['{"teleblock":{"ticksLeft":"2"}}', '{"poison":{"source-uid":12,"severity":"6"}}',
                                                        '{"frozen":{"ticksLeft":"2","applyImmunity":true}}'])))
            elif name == 'set':
                ops.append(op('set', fields=rng.choice([{'modal': True}, {'modal': False}, {'delay': 10 ** 6}, {'delay': 0}, {'lp': 1}])))
            elif name == 'logout':
                if kind == 'player':
                    ops.append(op('logout'))
            else:
                ops.append(op(name))
        yield case('sequences', [owner, player(name='Other', uid=12), npc(name='Spider')], ops, seed=number, uids={'12': 1},
                   dialogues={'swap': [3264731]}, sweep=rng.randrange(len(ops)) if number % 8 == 0 else None)


def cases():
    GROUPS.clear()
    for make in [arguments, poison, disease, restore, freezing, miasmic, simple, swap, antimacro, persistence, sequences]:
        yield from make()


def groups(cases):
    out = {}
    for case in cases:
        out[case['group']] = out.get(case['group'], 0) + 1
    return dict(sorted(out.items()))
