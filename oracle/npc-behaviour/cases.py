"""The designed npc-behaviour corpus.

`cases()` returns (case, sweep) pairs: when sweep is an operation index, the
generator adds one case per recorded call of that operation in which that call
fails. Entity 0 is always the NPC under test. The clock starts at `now`
(default 1,000,000,000 ms) and `advance` moves it 600 ms per tick.
"""

HOME = [3200, 3200, 0]
NOW = 1000000000
TEN_MINUTES = 600000


def op(kind, **kw):
    return dict(op=kind, **kw)


def npc(**kw):
    spec = dict(kind='npc', id=kw.pop('id', 100), location=kw.pop('location', HOME))
    spec.update(kw)
    return spec


def player(name, **kw):
    spec = dict(kind='player', name=name, location=kw.pop('location', [3201, 3201, 0]))
    spec.update(kw)
    return spec


HANDLER = dict(radius=4, chance=2)


def aggressor(**kw):
    """An aggressive NPC with a default handler."""
    kw.setdefault('aggressive', True)
    kw.setdefault('handler', dict(HANDLER))
    return npc(**kw)


def region_of(location):
    x, y = location[0], location[1]
    return (x >> 6 << 8) | (y >> 6)


REGION = str(region_of([3201, 3201, 0]))


def groups(cases):
    out = {}
    for case in cases:
        name = case.get('group', 'other')
        out[name] = out.get(name, 0) + 1
    return dict(sorted(out.items()))


def cases():
    out = []

    def add(group, entities, ops, sweep=None, **kw):
        case = dict(group=group, entities=entities, ops=ops, **kw)
        out.append((case, sweep))

    select_cases(add)
    target_cases(add)
    tolerance_cases(add)
    behaviour_cases(add)
    attackable_cases(add)
    impact_cases(add)
    lifecycle_cases(add)
    tick_cases(add)
    return out


def select_cases(add):
    # Seeds 0..15 cover every first nextInt(10) value; each chance ratio boundary.
    for seed in range(16):
        for chance in [-1, 0, 2, 6, 9, 10]:
            add('select', [aggressor(handler=dict(HANDLER, chance=chance)), player('a')], [op('select')], seed=seed, mathSeed=seed)
    # Pause boundary and the interaction lock.
    for pause in [99, 100, 101]:
        for locked in [False, True]:
            add('select', [aggressor(handler=dict(HANDLER, pause=pause, chance=10)), player('a')], [op('select')],
                answers={'interaction_locked:0': locked})
    # Target switching against an existing fight, and the death check.
    for switching in [False, True]:
        for attacking in [False, True]:
            for dead in [False, True]:
                add('select', [aggressor(handler=dict(HANDLER, switching=switching, chance=10), attacking=attacking,
                                         victim=2 if attacking else None), player('a'), player('b')],
                    [op('select')], seed=3, mathSeed=5, answers={'is_dead:0': dead})
    # The chosen player's own refusals: ignore_aggression, rights, allow_admin_aggression, null rights.
    for ignore in [None, False, True]:
        for rights in ['REGULAR_PLAYER', 'PLAYER_MODERATOR', 'ADMINISTRATOR', None]:
            for allow in [None, True]:
                attrs = {}
                if ignore is not None:
                    attrs['ignore_aggression'] = ignore
                if allow is not None:
                    attrs['allow_admin_aggression'] = allow
                add('select', [aggressor(handler=dict(HANDLER, chance=10)), player('a', attrs=attrs)], [op('select')],
                    answers={'rights:1': rights})
    # No candidate: the answer is isAttacking.
    for attacking in [False, True]:
        add('select', [aggressor(handler=dict(HANDLER, chance=10), attacking=attacking, victim=1 if attacking else None), player('a')],
            [op('select')], answers={'local_players': [[]]})
    # Radius values handed to the local query, and the wilderness behaviour.
    for radius in [-1, 0, 4, 64]:
        add('select', [aggressor(handler=dict(HANDLER, chance=10, radius=radius)), player('a')], [op('select')])
    for skull in [False, True]:
        for invisible in [False, True]:
            add('select', [aggressor(handler=dict(HANDLER, chance=10, behaviour='wilderness')), player('a')], [op('select')],
                answers={'skull_wilderness:1': skull, 'player_invisible:1': invisible})
    # Sweeps: every call of a full selection fails once, attacking and not.
    add('select', [aggressor(handler=dict(HANDLER, chance=10)), player('a'), player('b', attrs={'combat-time': NOW + 5})], [op('select')], 0,
        mathSeed=11)
    add('select', [aggressor(handler=dict(HANDLER, chance=10, switching=True), attacking=True, victim=2), player('a'), player('b')],
        [op('select')], 0, mathSeed=12)
    add('select', [aggressor(handler=dict(HANDLER, chance=10, behaviour='wilderness'), **{'class': 'recording'}), player('a')],
        [op('select')], 0, answers={'skull_wilderness:1': True})
    # A missing handler is a null dereference.
    add('select', [npc(aggressive=True), player('a')], [op('select')])


def target_cases(add):
    # Several candidates: priority flags (in combat, interaction lock, an aggressor fighting them) against Math.random.
    for seed in range(12):
        players = [player('a'), player('b', attrs={'combat-time': NOW + 1}), player('c'), player('d', attrs={'aggressor': 'e5'}),
                   npc(id=7, location=[3205, 3205, 0], victim=4)]
        add('targets', [aggressor(handler=dict(HANDLER, chance=10))] + players, [op('select')], mathSeed=seed,
            answers={'interaction_locked:3': True, 'is_multi_zone:2': True, 'local_players': [[1, 2, 3, 4]]})
    for seed in range(6):
        add('targets', [aggressor(handler=dict(HANDLER, chance=10)), player('a'), player('b'), player('c')],
            [op('logical', targets=[1, 2, 3]), op('logical', targets=[]), op('logical', targets=[3, 3])], mathSeed=seed)
    # getPriorityFlag boundaries one at a time.
    for combat_time in [NOW - 1, NOW, NOW + 1]:
        for locked in [False, True]:
            for aggressor_victim in [None, 1, 2]:
                attrs = {'combat-time': combat_time}
                entities = [aggressor(), player('a', attrs=attrs), player('b')]
                if aggressor_victim is not None:
                    entities.append(npc(id=9, victim=aggressor_victim))
                    attrs['aggressor'] = 'e3'
                add('targets', entities, [op('priority', target=1)], answers={'interaction_locked:1': locked})
    add('targets', [aggressor(), player('a', attrs={'combat-time': NOW + 9, 'aggressor': 'e2'}), npc(id=9, victim=1)],
        [op('priority', target=1)], 0, answers={'interaction_locked:1': True})
    # getPossibleTargets over a mixed list, both behaviours.
    for behaviour in ['default', 'wilderness']:
        add('targets', [aggressor(), player('a'), player('b', attrs={'combat-time': NOW + 3}), player('c')],
            [op('possible', behaviour=behaviour, radius=6)],
            answers={'is_active:3': False, 'skull_wilderness:1': True, 'skull_wilderness:2': True})
    add('targets', [aggressor(), player('a'), player('b')], [op('possible', behaviour='default', radius=6)], 0,
        answers={'local_players': [[2, 1]]})


def tolerance_cases(add):
    region = REGION
    # Region.isTolerated around the strict ten-minute boundary, a null entry, and its gates.
    for entry in [None, NOW, NOW - TEN_MINUTES, NOW - TEN_MINUTES - 1, 'null']:
        for allow in [True, False]:
            for can_tolerate in [None, False, True]:
                for wild in [False, True]:
                    regions = {}
                    if entry is not None:
                        regions = {region: {'tolerances': {'a': None if entry == 'null' else entry}}}
                    answers = {'wilderness_in_zone:0': wild}
                    if can_tolerate is not None:
                        answers['config:0:can_tolerate'] = can_tolerate
                    add('tolerance', [aggressor(handler=dict(HANDLER, allowTolerance=allow)), player('a')],
                        [op('can_select', behaviour='default', target=1), op('allow_tolerance')], regions=regions, answers=answers)
    add('tolerance', [aggressor(), player('a')], [op('can_select', behaviour='default', target=1)], 0,
        regions={region: {'tolerances': {'a': NOW - TEN_MINUTES - 1}}})
    add('tolerance', [aggressor(), player('a')], [op('can_select', behaviour='default', target=1)], 0,
        regions={region: {'tolerances': {'a': NOW}}})
    # No handler on the NPC: tolerance is not consulted. An NPC target: not consulted either.
    add('tolerance', [npc(), player('a')], [op('can_select', behaviour='default', target=1)],
        regions={region: {'tolerances': {'a': NOW - TEN_MINUTES - 1}}})
    add('tolerance', [aggressor(), npc(id=5, location=[3201, 3201, 0])], [op('can_select', behaviour='default', target=1)])
    # Combat level against twice the NPC's, with the shift wrapping.
    for mine, theirs in [(3, 6), (3, 7), (3, 5), (0, 0), (0, 1), (-1, -2), (-1, -3), (1073741824, 0), (1073741824, -2147483648),
                         (2147483647, 2147483647), (1610612736, 5)]:
        add('tolerance', [aggressor(), player('a')], [op('can_select', behaviour='default', target=1)],
            answers={'current_combat_level:0': mine, 'current_combat_level:1': theirs})
    # Active, dead, multi-zone and combat time.
    for active in [True, False]:
        for dead in [False, True]:
            for multi in [False, True]:
                for combat_time in [None, NOW, NOW + 1]:
                    attrs = {} if combat_time is None else {'combat-time': combat_time}
                    for behaviour in ['default', 'wilderness']:
                        add('tolerance', [aggressor(), player('a', attrs=attrs)], [op('can_select', behaviour=behaviour, target=1)],
                            answers={'is_active:1': active, 'is_dead:1': dead, 'is_multi_zone:1': multi, 'skull_wilderness:1': True})
    # The wilderness behaviour with a content class that refuses the target, and an NPC target.
    for answer in [True, False]:
        add('tolerance', [aggressor(**{'class': 'recording'}), player('a')], [op('can_select', behaviour='wilderness', target=1)],
            answers={'class_hook:canSelectTarget': answer, 'skull_wilderness:1': True})
    add('tolerance', [aggressor(), npc(id=5, location=[3201, 3201, 0])], [op('can_select', behaviour='wilderness', target=1)])
    add('tolerance', [aggressor(), npc(id=5, location=[3201, 3201, 0], respawn=101)], [op('can_select', behaviour='wilderness', target=1)])
    add('tolerance', [aggressor(**{'class': 'recording'}), player('a')], [op('can_select', behaviour='wilderness', target=1)], 0,
        answers={'skull_wilderness:1': True})


def behaviour_cases(add):
    hooks = [dict(name='tick'), dict(name='canBeAttackedBy', other=1, style='MELEE', message=True),
             dict(name='canBeAttackedBy', other=2, style='RANGE', message=False), dict(name='canBeAttackedBy', other=1, style=None),
             dict(name='shouldIgnoreMultiRestrictions', other=1), dict(name='shouldIgnoreMultiRestrictions', other=None),
             dict(name='getXpMultiplier', other=1), dict(name='getPathfinderOverride'), dict(name='clip', z=0, x=3200, y=3201),
             dict(name='onCreation'), dict(name='onRemoval'), dict(name='onRespawn'), dict(name='onDeathStarted', other=1),
             dict(name='onDeathStarted', other=None), dict(name='onDeathFinished', other=2), dict(name='beforeAttackFinalized', other=1),
             dict(name='onDropTableRolled', other=1)]
    for registered in [False, True]:
        for actions in [(True, True), (False, True), (False, False), (True, False)]:
            answers = {'has_action:0:attack': actions[0], 'has_action:0:smash-ice': actions[1], 'clipping_flag': 256,
                       'behaviour_hook:tick': False, 'behaviour_hook:canBeAttackedBy': False}
            ops = [op('hook', **h) for h in hooks]
            add('behaviour', [npc(), player('a'), npc(id=5)], ops, answers=answers,
                behaviours=[dict(ids=[100], tag='content')] if registered else [])
    # The registry: later registrations replace an id; unregistered ids get the shared default.
    add('behaviour', [npc(id=6)], [op('for_id', id=5), op('register', ids=[5, 6], tag='a'), op('for_id', id=5), op('for_id', id=6),
                                   op('register', ids=[6, 7], tag='b'), op('for_id', id=5), op('for_id', id=6), op('for_id', id=7),
                                   op('for_id', id=8), op('refresh_behavior')])
    # The behaviour's swing handler override, and isIgnoreMultiBoundaries.
    for registered in [False, True]:
        for answer in [None, 'RANGE', 'MAGIC']:
            for style in ['MELEE', 'RANGE', 'MAGIC', None]:
                add('behaviour', [npc(style=style), player('a')], [op('swing_handler', swing=True), op('ignore_multi', victim=1)],
                    behaviours=[dict(ids=[100], tag='content')] if registered else [],
                    answers={'behaviour_hook:getSwingHandlerOverride': answer, 'behaviour_hook:shouldIgnoreMultiRestrictions': True})
    add('behaviour', [npc(), player('a')], [op('ignore_multi', entity=1, victim=0)])
    add('behaviour', [npc(style='MAGIC'), player('a')], [op('swing_handler')], 0, behaviours=[dict(ids=[100], tag='content')],
        answers={'behaviour_hook:getSwingHandlerOverride': 'RANGE'})


def attackable_cases(add):
    # NPC victims: invisibility, the slayer task message, the behaviour, death and both zone monitors.
    for respawn in [-1, 100, 101]:
        for active in [True, False]:
            for invisible in [False, True]:
                add('attackable', [npc(respawn=respawn, invisible=invisible), player('a')],
                    [op('attackable', victim=0, attacker=1, style='MELEE', message=True), op('invisible'), op('hidden', player=1),
                     op('continue', target=1, style='MELEE')], answers={'is_active:0': active})
    for task in [None, 1, 50]:
        for level in [1, 49, 50, 99]:
            for message in [False, True]:
                add('attackable', [npc(task=task), player('a')], [op('attackable', victim=0, attacker=1, style='RANGE', message=message)],
                    answers={'level:1': level})
    for actions in [(True, True), (False, True), (False, False)]:
        for attacker in ['player', 'npc']:
            for dead in [False, True]:
                for zones in [(True, True), (False, True), (True, False)]:
                    entities = [npc(), player('a') if attacker == 'player' else npc(id=5)]
                    add('attackable', entities, [op('attackable', victim=0, attacker=1, style='MAGIC', message=False)],
                        answers={'has_action:0:attack': actions[0], 'has_action:0:smash-ice': actions[1], 'is_dead:0': dead,
                                 'zone_continue_attack:1': zones[0], 'zone_continue_attack:0': zones[1]})
    add('attackable', [npc(), player('a')], [op('attackable', victim=0, attacker=1, style=None, message=True)])
    for answer in [True, False]:
        add('attackable', [npc(), player('a')], [op('attackable', victim=0, attacker=1, style='MELEE', message=True)],
            behaviours=[dict(ids=[100], tag='content')], answers={'behaviour_hook:canBeAttackedBy': answer})
    add('attackable', [npc(task=60), player('a')], [op('attackable', victim=0, attacker=1, style='MELEE', message=True)], 0,
        answers={'level:1': 5})
    # A familiar victim is an NPC.
    add('attackable', [dict(kind='familiar', id=6, location=HOME), player('a')], [op('attackable', victim=0, attacker=1, style='MELEE', message=False)])
    # Player victims: an NPC without an attack option, content that ignores restrictions, and player versus player.
    for action in [True, False]:
        for klass in ['plain', 'recording']:
            for ignore in [False, True]:
                add('attackable', [npc(**{'class': klass}), player('a')], [op('attackable', victim=1, attacker=0, style='MELEE', message=False)],
                    answers={'has_action:0:attack': action, 'class_hook:isIgnoreAttackRestrictions': ignore})
    for skulls in [(False, False), (True, False), (False, True), (True, True)]:
        for settings in [True, False]:
            for pvp in [False, True]:
                for protections in [(False, False), (True, False), (False, True)]:
                    for pvpable in [False, True]:
                        answers = {'skull_wilderness:1': skulls[0], 'skull_wilderness:2': skulls[1], 'settings': settings,
                                   'wild_pvp_enabled': pvp, 'wilderness_protection:1': protections[0],
                                   'wilderness_protection:2': protections[1], 'zone_is_pvpable:2': pvpable}
                        add('attackable', [npc(), player('a'), player('b')],
                            [op('attackable', victim=2, attacker=1, style='MELEE', message=True),
                             op('continue', entity=2, target=1, style='RANGE', message=False)], answers=answers)
    # Player.continueAttack against NPCs: the shown NPC's options and the ignore rule.
    for shown in [None, 3]:
        for actions in [(True, False), (False, True), (False, False)]:
            for klass in ['plain', 'recording']:
                add('attackable', [npc(**{'class': klass}), player('a'), player('b'), npc(id=8)],
                    [op('continue', entity=1, target=0, style='MELEE', message=True)],
                    answers={'shown_npc:0': shown, f'has_action:{shown or 0}:attack': actions[0],
                             f'has_action:{shown or 0}:smash-ice': actions[1], 'class_hook:isIgnoreAttackRestrictions': True})
    add('attackable', [npc(), player('a'), player('b')], [op('continue', entity=2, target=1, style='MELEE', message=True)], 0,
        answers={'skull_wilderness:1': True, 'skull_wilderness:2': True, 'wild_pvp_enabled': True})
    add('attackable', [npc(**{'class': 'recording'}), player('a')], [op('attackable', victim=1, attacker=0, style='MELEE', message=False)], 0,
        answers={'has_action:0:attack': False, 'class_hook:isIgnoreAttackRestrictions': True})


def impact_cases(add):
    # checkImpact: the safespot rule at its tick boundaries, and the slayer task rules.
    for last in [89, 90, 59, 60, 100]:
        for safespot in [False, True]:
            add('impact', [npc(lastSent=last), player('a')],
                [op('check_impact', state=dict(attacker=1, victim=0, estimated=6, secondary=2, poison=4, style='MELEE'))],
                answers={'config:0:safespot': safespot})
    for task in [None, 30]:
        for level in [29, 30, 31]:
            for style in ['MELEE', 'MAGIC']:
                for attacker in ['player', 'familiar', 'npc']:
                    if attacker == 'player':
                        second = player('a')
                    elif attacker == 'familiar':
                        second = dict(kind='familiar', id=6, location=[3201, 3201, 0])
                    else:
                        second = npc(id=5)
                    add('impact', [npc(task=task), second, player('owner')],
                        [op('check_impact', state=dict(attacker=1, victim=0, estimated=5, secondary=3, poison=2, style=style))],
                        answers={'level:1': level, 'level:2': level, 'familiar_owner:1': 2})
    add('impact', [npc(task=30), dict(kind='familiar', id=6, location=[3201, 3201, 0])],
        [op('check_impact', state=dict(attacker=1, victim=0, estimated=5, style='MELEE'))], answers={'familiar_owner:1': None})
    add('impact', [npc(), player('a')], [op('check_impact', state=dict(attacker=None, victim=0, estimated=5, style='MELEE'))])
    add('impact', [npc(lastSent=10), player('a')], [op('check_impact', state=dict(attacker=None, victim=0, estimated=5, style='MELEE'))],
        answers={'config:0:safespot': True})
    add('impact', [npc(lastSent=10), player('a')], [op('check_impact', state=dict(attacker=1, victim=None, estimated=5, style='MELEE'))],
        answers={'config:0:safespot': True})
    add('impact', [player('victim'), player('a')], [op('check_impact', state=dict(attacker=1, victim=0, estimated=5, style='MELEE'))])
    add('impact', [npc(task=30), player('a')],
        [op('check_impact', state=dict(attacker=1, victim=0, estimated=5, secondary=3, style='MAGIC'))],
        behaviours=[dict(ids=[100], tag='content')], answers={'level:1': 1, 'behaviour_state': {'estimated': 9, 'secondary': 0}})
    add('impact', [npc(lastSent=10, task=30), player('a')],
        [op('check_impact', state=dict(attacker=1, victim=0, estimated=5, secondary=3, style='MAGIC'))], 0,
        answers={'config:0:safespot': True, 'level:1': 1})
    # onImpact: death neutralises, then the retaliation conditions.
    for kind in ['npc', 'player']:
        for dead in [False, True]:
            for retaliating in [True, False]:
                for attacking in [False, True]:
                    for locked in [False, True]:
                        for next_attack in [99, 100]:
                            victim = npc(attacking=attacking, nextAttack=next_attack) if kind == 'npc' else \
                                player('v', attacking=attacking, nextAttack=next_attack)
                            add('impact', [victim, player('a')],
                                [op('on_impact', attacker=1, state=dict(attacker=1, victim=0, estimated=3, secondary=1, style='MELEE'))],
                                answers={'is_dead:0': dead, 'is_retaliating:0': retaliating, 'interaction_locked:0': locked})
        for path in [False, True]:
            for moving in [False, True]:
                victim = npc() if kind == 'npc' else player('v')
                add('impact', [victim, player('a')], [op('on_impact', attacker=1, state=dict(attacker=1, victim=0, estimated=3, style='MELEE'))],
                    answers={'walking_has_path:0': path, 'pulse_manager_is_moving:0': moving})
    add('impact', [npc(), player('a')], [op('on_impact', attacker=None, state=dict(attacker=None, victim=0, estimated=3, style='MELEE'))])
    add('impact', [player('v'), player('a')], [op('on_impact', attacker=None, state=dict(attacker=None, victim=0, estimated=0, style='MELEE'))])
    add('impact', [player('v'), player('a')], [op('on_impact', attacker=None, state=dict(attacker=None, victim=0, estimated=4, style='MELEE'))],
        answers={'equipment_new:0': 2550})
    add('impact', [player('v', attrs={'vengeance': True}), player('a')],
        [op('on_impact', attacker=None, state=dict(attacker=None, victim=0, estimated=4, style='MELEE'))])
    # Player.onImpact: vengeance, the ring of recoil, secondary hits.
    for estimated in [-1, 0, 1, 4, 7]:
        for vengeance in [None, False, True]:
            for ring in [0, 2550]:
                for secondary in [-1, 0, 3]:
                    attrs = {} if vengeance is None else {'vengeance': vengeance}
                    add('impact', [player('v', attrs=attrs), npc()],
                        [op('on_impact', attacker=1, state=dict(attacker=1, victim=0, estimated=estimated, secondary=secondary, style='RANGE'))],
                        answers={'equipment_new:0': ring, 'is_retaliating:0': False})
    add('impact', [npc(), player('a')], [op('on_impact', attacker=1, state=dict(attacker=1, victim=0, estimated=3, style='MELEE'))],
        behaviours=[dict(ids=[100], tag='content')], answers={'behaviour_state': {'estimated': 0}})
    add('impact', [player('v', attrs={'vengeance': True}), npc()],
        [op('on_impact', attacker=1, state=dict(attacker=1, victim=0, estimated=8, secondary=2, style='MELEE'))], 0,
        answers={'equipment_new:0': 2550})
    add('impact', [npc(), player('a')], [op('on_impact', attacker=1, state=dict(attacker=1, victim=0, estimated=3, style='MELEE'))], 0)
    # sendImpact and inCombat.
    add('impact', [npc(), player('a')], [op('send_impact'), op('advance', ticks=3), op('send_impact', entity=1)])
    add('impact', [npc()], [op('send_impact')], 0)
    for combat_time in [None, NOW - 1, NOW, NOW + 1, -9223372036854775808, 9223372036854775807]:
        attrs = {} if combat_time is None else {'combat-time': combat_time}
        add('impact', [npc(attrs=attrs), player('a', attrs=attrs)], [op('in_combat'), op('in_combat', entity=1)])


def lifecycle_cases(add):
    # onRegionInactivity: not returning, returning with and without its pulse, a dynamic region.
    # The return pulse is the original anonymous MovementPulse, made by handleTickActions for an NPC beyond 1.5 radii.
    region = str(region_of(HOME))
    far = dict(location=[3230, 3200, 0], spawn=HOME, radius=11)
    for dynamic in [False, True]:
        regions = {region: {'dynamic': dynamic}}
        add('lifecycle', [npc(location=[3203, 3204, 0], spawn=HOME)], [op('inactivity')], regions=regions)
        add('lifecycle', [npc(**far)], [op('actions'), op('inactivity'), op('tick')], 1, regions=regions)
        add('lifecycle', [npc(**far)], [op('actions'), op('set', fields=dict(returnPulse=None)), op('inactivity')], 2, regions=regions)
        add('lifecycle', [npc(location=[3203, 3204, 0], spawn=HOME, returning=True)], [op('inactivity')], regions=regions)
    add('lifecycle', [npc(location=[3203, 3204, 0], spawn=None, returning=True)], [op('inactivity')])
    # fullRestore.
    add('lifecycle', [npc()], [op('restore')], 0)
    # setDefaultBehavior and configureBossData.
    for aggressive in [False, True]:
        for level in [-1, 0, 1, 138]:
            for radius in [None, 0, 64]:
                answers = {'definition_combat_level:0': level}
                if radius is not None:
                    answers['config:0:agg_radius'] = radius
                add('lifecycle', [npc(aggressive=aggressive)], [op('default_behavior')], answers=answers)
                add('lifecycle', [npc(aggressive=aggressive)], [op('boss'), op('allow_tolerance')], answers=answers)
    add('lifecycle', [npc(aggressive=True)], [op('default_behavior')], 0, answers={'definition_combat_level:0': 5})
    add('lifecycle', [npc(aggressive=False)], [op('boss')], 0, answers={'definition_combat_level:0': 5})
    add('lifecycle', [npc(aggressive=True, handler=dict(HANDLER))], [op('boss')], answers={'definition_combat_level:0': 0})
    # The handler's own members.
    for ticks in [-5, 0, 7, 2147483647]:
        add('lifecycle', [aggressor()], [op('pause', ticks=ticks), op('advance', ticks=7), op('pause', ticks=ticks)], ticks=2147483640)
    for index in [-1, 0, 1999, 2000]:
        add('lifecycle', [aggressor()], [op('remove_tolerance', index=index)])
    add('lifecycle', [aggressor()], [op('pause', ticks=3)], 0)


def tick_cases(add):
    region = REGION
    loop = lambda n: [x for _ in range(n) for x in (op('tick'), op('advance'))]
    # Aggression through the original tick: a player inside the radius is attacked once the draw allows.
    for seed in [0, 1, 2, 3, 530]:
        for sticky in [True, False]:
            add('tick', [aggressor(), player('a'), player('b', location=[3203, 3199, 0])], loop(30), seed=seed, mathSeed=seed, sticky=sticky)
    # Tolerance timers over a long run: from the entry time, aggression stops strictly after ten minutes.
    for allow in [True, False]:
        add('tick', [aggressor(handler=dict(HANDLER, chance=10, allowTolerance=allow)), player('a')], loop(1004), sticky=False,
            regions={region: {'tolerances': {'a': NOW - 5 * 600}}})
    add('tick', [aggressor(handler=dict(HANDLER, chance=10)), player('a')], loop(1003), sticky=False,
        regions={region: {'tolerances': {'a': NOW}}}, answers={'config:0:can_tolerate': False})
    # Multi versus single combat: a target already in combat is chosen only inside a multi-way zone.
    for multi in [False, True]:
        add('tick', [aggressor(handler=dict(HANDLER, chance=10)), player('a', attrs={'combat-time': NOW + 3000}), player('b')], loop(12),
            mathSeed=4, sticky=False, answers={'is_multi_zone:1': multi, 'is_multi_zone:2': multi, 'local_players': [[1]] * 12})
    # Respawn timing with content hooks: the teleport two ticks before, then onRespawn, the class hook and fullRestore.
    for registered in [False, True]:
        add('tick', [aggressor(respawn=105, respawning=True, location=[3210, 3210, 0], spawn=HOME, **{'class': 'recording'}), player('a')],
            loop(9), behaviours=[dict(ids=[100], tag='content')] if registered else [], seed=9)
    add('tick', [aggressor(respawn=102, respawning=True, **{'class': 'recording'})], [op('tick'), op('advance', ticks=2), op('tick')], 2,
        behaviours=[dict(ids=[100], tag='content')])
    # Region inactivity inside the tick, while returning to spawn.
    add('tick', [aggressor(location=[3230, 3200, 0], spawn=HOME, radius=11, walks=True), player('a')],
        loop(3) + [op('tick')], answers={f'region_active:{region_of(HOME)}': [True, True, False, True]}, seed=4)
    add('tick', [aggressor(location=[3230, 3200, 0], spawn=HOME, radius=11), player('a')], [op('tick'), op('advance'), op('tick')], 2,
        answers={f'region_active:{region_of(HOME)}': [True, False]})
    # The behaviour's tick answer stops the NPC's own actions; wandering NPCs draw as they walk.
    add('tick', [aggressor(), player('a')], loop(3), behaviours=[dict(ids=[100], tag='content')], answers={'behaviour_hook:tick': [True, False, True]})
    for seed in [0, 4096, 4097]:
        add('tick', [aggressor(walks=True, handler=dict(HANDLER, chance=10)), player('a', location=[3220, 3220, 0])], loop(20),
            seed=seed, mathSeed=seed, answers={'local_players': [[]] * 20})
    # Return to spawn pauses aggression for radius + 1 ticks.
    add('tick', [aggressor(location=[3220, 3200, 0], spawn=HOME, radius=6, handler=dict(HANDLER, chance=10)), player('a')], loop(10),
        answers={'return_running:1': [True, True, True, False]})
    add('tick', [aggressor(location=[3220, 3200, 0], spawn=HOME, radius=6, handler=dict(HANDLER, chance=10)), player('a')], [op('tick')], 0)
    add('tick', [aggressor(handler=dict(HANDLER, chance=10)), player('a')], [op('tick')], 0, mathSeed=2)
