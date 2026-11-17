"""The designed teleport corpus.

Every case starts from fresh entities (entity 0 is the teleporting one) and
plain destination Location objects, then drives operations against the
unchanged classes. A world tick is the three operations `pulse` (the runner),
`move` (every walking queue) and `end` (the reset of the teleporting and scene
flags, then the clock). `cases()` returns (case, sweep) pairs: when sweep is an
operation index, generate.py adds one copy of the case per recorded call of
that operation, failing that call.
"""

TYPES = ['NORMAL', 'ANCIENT', 'LUNAR', 'TELETABS', 'OBELISK', 'TELE_OTHER', 'FAIRY_RING', 'PURO_PURO', 'ECTOPHIAL', 'CHRISTMAS',
         'CABBAGE', 'ENTRANA_MAGIC_DOOR', 'MINIGAME', 'INSTANT', 'RC_GUILD']
# The number of pulse() calls each type's body takes to return true.
LENGTH = dict(NORMAL=5, ANCIENT=6, LUNAR=5, TELETABS=5, OBELISK=5, TELE_OTHER=5, FAIRY_RING=4, PURO_PURO=10, ECTOPHIAL=5,
              CHRISTMAS=5, CABBAGE=6, ENTRANA_MAGIC_DOOR=5, MINIGAME=10, INSTANT=5, RC_GUILD=6)
WILDY = 1 << 16 | 8
HOME = [3222, 3218, 0]
NEAR = [3210, 3424, 0]
FAR = [2964, 3378, 0]
CLOSE = [3225, 3220, 0]
# Animation lengths in frames of the definitions the corpus answers (each frame <= 100 counts frame * 20 ms).
DURATIONS = {'8939': [100, 50], '8941': [100, 100, 100], '1979': [100, 100], '1816': [60, 60, 60], '4731': [100], '4069': [30],
             '3265': [100, 100], '3266': [100, 100, 100, 100], '6601': [100, 100, 100, 100, 100, 100], '9984': [100, 100], '9986': [101, 100],
             '10100': None, '9013': 'missing', '7534': [], '10180': [100, 100], '10182': [100, 100, 100]}


def op(_operation, **kw):
    return dict(op=_operation, **kw)


def tick():
    return [op('pulse'), op('move'), op('end')]


def ticks(count):
    return [step for _ in range(count) for step in tick()]


def player(**kw):
    return dict(dict(kind='player', loc=HOME, last=HOME, spellbook=192), **kw)


def npc(**kw):
    return dict(dict(kind='npc', loc=HOME), **kw)


def case(group, ops, entities=None, dests=None, **kw):
    return dict(dict(group=group, entities=entities or [player()], dests=dests if dests is not None else [NEAR, CLOSE],
                     durations=DURATIONS, ticks=kw.pop('start', 100), ops=ops), **kw)


def complete():
    out = []
    for t in TYPES:
        for who, entity, form in [('player', player(), dict(form=2)), ('npc', npc(), dict(form=2)),
                                  ('force', player(), dict(form=3, tt=-1)), ('item', player(), dict(form=3, tt=1)),
                                  ('object', player(), dict(form=3, tt=2))]:
            body = [op('send', loc=0, type=t, **form), op('teleporting')]
            for _ in range(LENGTH[t] + 2):
                body += [op('pulse'), op('move'), op('teleporting'), op('end')]
            out.append((case('complete-' + who, body, [entity]), None))
    for book, kind in [(192, 'player'), (193, 'player'), (430, 'player'), (0, 'player'), (430, 'npc')]:
        entity = player(spellbook=book) if kind == 'player' else npc()
        out.append((case('default-type', [op('send', loc=0, form=1), *ticks(8)], [entity]), None))
    for book in [192, 193, 430, -1, 2147483647]:
        out.append((case('table', [op('table'), op('spellbook', value=book), op('get_type'), op('teleport_type'),
                                   op('set_teleport_type', value=-7), op('teleport_type')]), None))
    out.append((case('table', [op('get_type')], [npc()]), None))
    # The far destination flags a scene rebuild; the driver's end phase stands in for the rebuild packet.
    for t in ['NORMAL', 'FAIRY_RING', 'PURO_PURO']:
        out.append((case('far', [op('send', loc=0, type=t, form=2), *ticks(LENGTH[t] + 3)], dests=[FAR]), None))
    return out


def interruptions():
    out = []
    for t in TYPES:
        for at in range(LENGTH[t] + 1):
            before = [op('send', loc=0, type=t, form=2)] + ticks(at - 1 if at else 0) + ([op('pulse')] if at else [])
            after = ([op('move'), op('end')] if at else []) + ticks(LENGTH[t] + 2 - at)
            other = 'LUNAR' if t == 'NORMAL' else 'NORMAL'
            out.append((case('interrupt-stop', before + [op('stop'), op('teleporting')] + after), None))
            out.append((case('interrupt-resend', before + [op('send', loc=1, type=t, form=2)] + after), None))
            out.append((case('interrupt-other', before + [op('send', loc=1, type=other, form=2)] + after), None))
            out.append((case('interrupt-npc-stop', before + [op('stop')] + after, [npc()]), None))
        out.append((case('interrupt-clear', [op('send', loc=0, type=t, form=2), *ticks(2), op('clear'), *ticks(LENGTH[t])]), None))
        out.append((case('interrupt-start', [op('send', loc=0, type=t, form=2), op('pulse'), op('start'), op('teleporting'),
                                            op('move'), op('end'), *ticks(LENGTH[t] + 1), op('start'), op('teleporting'), *ticks(2),
                                            op('stop'), op('teleporting')]), None))
        out.append((case('interrupt-old', [op('send', loc=0, type=t, form=2), op('pulse'), op('send', loc=1, type=t, form=2),
                                          op('stop_pulse', pulse=0), *ticks(LENGTH[t] + 2)]), None))
    # Stopping before any send has no current teleport.
    out.append((case('interrupt-none', [op('stop'), op('start'), op('teleporting')]), None))
    return out


def blocks():
    out = []
    for kind in ['player', 'npc']:
        entity = player if kind == 'player' else npc
        for blocked in [False, True]:
            for t, tt in [('NORMAL', 0), ('NORMAL', 1), ('NORMAL', 2), ('NORMAL', 3), ('NORMAL', -1), ('NORMAL', WILDY),
                          ('OBELISK', 0), ('OBELISK', -1), ('OBELISK', WILDY), ('ANCIENT', WILDY)]:
                timers = ['teleblock'] if blocked else []
                out.append((case('block-timer', [op('send', loc=0, type=t, form=3, tt=tt), op('teleporting'), *ticks(2)],
                                 [entity(timers=timers)]), None))
            for t, tt in [('NORMAL', 0), ('NORMAL', -1), ('NORMAL', WILDY), ('OBELISK', 0), ('NORMAL', 1)]:
                setup = [op('lock_teleport', ticks=5)] + (ticks(6) if not blocked else [])
                out.append((case('block-lock', setup + [op('send', loc=0, type=t, form=3, tt=tt), *ticks(2)], [entity()]), None))
                restriction = 1 << 6 if blocked else 1 << 2
                out.append((case('block-restriction', [op('send', loc=0, type=t, form=3, tt=tt), *ticks(2)],
                                 [entity(zones=[0])], zones=[dict(restriction=restriction)]), None))
        for answers in [[[True]], [[False]], [[True], [True]], [[True], [False]], [[False], [True]]]:
            for t, tt in [('NORMAL', 0), ('NORMAL', -1), ('NORMAL', WILDY), ('OBELISK', 0)]:
                zones = [dict(answers=a) for a in answers]
                out.append((case('block-zone', [op('send', loc=0, type=t, form=3, tt=tt), *ticks(2)],
                                 [entity(zones=list(range(len(zones))))], zones=zones), None))
        # A zone that tele-blocks during its own teleport check: the second isTeleBlocked refuses (but not a forced teleport).
        for tt in [0, -1, 3]:
            out.append((case('block-second', [op('send', loc=0, type='NORMAL', form=3, tt=tt), *ticks(2)], [entity(zones=[0])],
                             zones=[dict(answers=[True])],
                             hooks=[dict(event='zone_teleport', actions=[op('timer', name='teleblock', on=True)])]), None))
        out.append((case('block-second', [op('send', loc=0, type='NORMAL', form=3, tt=0), *ticks(2)], [entity(zones=[0])],
                         zones=[dict(answers=[True])], hooks=[dict(event='zone_teleport', actions=[op('lock_teleport', ticks=3)])]), None))
        # A zone that removes itself from the live list while the monitor walks it.
        out.append((case('block-modified', [op('send', loc=0, type='NORMAL', form=2), *ticks(2)], [entity(zones=[0, 1])],
                         zones=[dict(answers=[True]), dict(answers=[True])],
                         hooks=[dict(event='zone_teleport', actions=[op('zone_remove', zone=0)])]), None))
        out.append((case('block-modified', [op('send', loc=0, type='NORMAL', form=2), *ticks(2)], [entity(zones=[0, 1])],
                         zones=[dict(answers=[True]), dict(answers=[True])],
                         hooks=[dict(event='zone_teleport', actions=[op('zone_remove', zone=1)])]), None))
        out.append((case('block-modified', [op('send', loc=0, type='NORMAL', form=2), *ticks(2)], [entity(zones=[0, 1, 2])],
                         zones=[dict(answers=[True]), dict(answers=[True]), dict(answers=[True])],
                         hooks=[dict(event='zone_teleport', actions=[op('zone_remove', zone=2)])]), None))
        # Tablet spell: true turns any type into TELETABS; null and false do not; an Integer fails the Boolean cast.
        for value in [True, False, None, 1, 'remove']:
            attrs = {} if value == 'remove' else {'tablet-spell': value}
            for t in ['NORMAL', 'ANCIENT']:
                out.append((case('tablet', [op('send', loc=0, type=t, form=2), *ticks(LENGTH['TELETABS'] + 1)], [entity(attrs=attrs)]), None))
        out.append((case('tablet', [op('send', loc=0, type=None, form=3, tt=0), *ticks(6)], [entity(attrs={'tablet-spell': True})]), None))
        # A null type fails at getPulse, after the checks, the delay, the dialogue and the walking reset.
        for tt in [0, -1, WILDY]:
            out.append((case('null-type', [op('send', loc=0, type=None, form=3, tt=tt), op('teleporting'), *ticks(2)], [entity()]), None))
        # A null destination fails when the pulse copies it (FAIRY_RING stores it as given).
        for t in TYPES:
            out.append((case('null-destination', [op('send', loc=None, type=t, form=2), *ticks(LENGTH[t] + 2)], [entity()]), None))
        # A null entity location fails in the walking queue's reset.
        out.append((case('null-location', [op('send', loc=0, type='NORMAL', form=2), *ticks(2)], [entity(loc=None)]), None))
    return out


def zones():
    out = []
    # Out of a zone that refuses, then the zone is left on arrival elsewhere and another zone restricts teleports.
    out.append((case('zones', [op('send', loc=0, type='NORMAL', form=2), op('zone_remove', zone=0), op('send', loc=0, type='NORMAL', form=2),
                               *ticks(6), op('send', loc=1, type='NORMAL', form=2), *ticks(2), op('send', loc=1, type='NORMAL', form=3, tt=-1),
                               *ticks(6)],
                     [player(zones=[0])], zones=[dict(answers=[False]), dict(restriction=1 << 6)],
                     hooks=[dict(event='zone_update', actions=[op('zone_add', zone=1)])]), None))
    # Into a zone whose entry starts another teleport, stops the current one or adds a tele-block.
    for actions in [[op('send', loc=1, type='LUNAR', form=2)], [op('stop')], [op('timer', name='teleblock', on=True)],
                    [op('send', loc=1, type='FAIRY_RING', form=3, tt=-1)]]:
        for t in ['NORMAL', 'FAIRY_RING', 'PURO_PURO']:
            out.append((case('zones-arrival', [op('send', loc=0, type=t, form=2), *ticks(LENGTH[t] + 7), op('send', loc=0, type='NORMAL', form=2),
                                               *ticks(6)],
                             [player()], hooks=[dict(event='zone_update', actions=actions)]), None))
    # A zone that starts another teleport from inside its own teleport check.
    out.append((case('zones-reentry', [op('send', loc=0, type='NORMAL', form=2), *ticks(8)], [player(zones=[0])],
                     zones=[dict(answers=[True, True])],
                     hooks=[dict(event='zone_teleport', actions=[op('send', loc=1, type='ANCIENT', form=2)])]), None))
    # Two entities: an NPC and a player teleporting in the same ticks.
    out.append((case('zones-two', [op('send', loc=0, type='NORMAL', form=2), op('send', e=1, loc=1, type='LUNAR', form=2), *ticks(8)],
                     [player(), npc(loc=CLOSE)]), None))
    return out


def mutations():
    out = []
    for t in ['NORMAL', 'FAIRY_RING', 'ANCIENT', 'PURO_PURO']:
        for at in [0, 2, LENGTH[t] - 1, LENGTH[t] + 1]:
            body = [op('send', loc=0, type=t, form=2), *ticks(at), op('mutate', d=0, to=[3300, 3300, 1]), *ticks(LENGTH[t] + 3 - at),
                    op('mutate', d=0, to=[3301, 3302, 2]), op('teleporting')]
            out.append((case('mutate', body), None))
    return out


def priorities():
    out = []
    # A high-priority end animation still playing rejects the next type's MID animate (FAIRY_RING) but not a forced one.
    for first in ['NORMAL', 'LUNAR', 'TELETABS', 'CABBAGE', 'RC_GUILD']:
        for second in ['FAIRY_RING', 'NORMAL', 'OBELISK']:
            out.append((case('priority', [op('send', loc=0, type=first, form=2), *ticks(LENGTH[first]), op('send', loc=1, type=second, form=2),
                                          *ticks(LENGTH[second] + 1)]), None))
    for start in [0, 2147483640, -5]:
        out.append((case('clock', [op('send', loc=0, type='NORMAL', form=2), *ticks(7), op('lock_forever'), op('send', loc=1, type='OBELISK', form=2),
                                   *ticks(6)], start=start), None))
    return out


def failures():
    out = []
    for t in ['NORMAL', 'FAIRY_RING', 'TELETABS', 'MINIGAME', 'OBELISK']:
        out.append((case('sweep-send', [op('send', loc=0, type=t, form=2), *ticks(LENGTH[t] + 1)], [player(zones=[0])],
                         zones=[dict(answers=[True])]), 0))
        for at in range(LENGTH[t]):
            body = [op('send', loc=0, type=t, form=2), *ticks(at), op('pulse'), op('move'), op('end'), *ticks(LENGTH[t] + 1 - at)]
            out.append((case('sweep-pulse', body), 1 + 3 * at))
    out.append((case('sweep-send', [op('send', loc=0, type='NORMAL', form=3, tt=0), *ticks(2)], [player(timers=['teleblock'])]), 0))
    out.append((case('sweep-send', [op('send', loc=0, form=1), *ticks(2)], [player(spellbook=193)]), 0))
    for t in ['NORMAL', 'MINIGAME', 'TELETABS']:
        out.append((case('sweep-stop', [op('send', loc=0, type=t, form=2), op('pulse'), op('stop'), *ticks(2)]), 2))
        out.append((case('sweep-start', [op('send', loc=0, type=t, form=2), op('pulse'), op('start'), *ticks(2)]), 2))
    return out


def cases():
    return complete() + interruptions() + blocks() + zones() + mutations() + priorities() + failures()
