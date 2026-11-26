"""Run CombatPulse's private MovementPulse (the combat chase) on JDK 11 and record what it does.

The chase is the unchanged MovementPulse in the configuration CombatPulse gives it: `MovementPulse(entity, null)` made
when the combat pulse is constructed (so its destination flag stays null), `setLast(null)` and
`setDestination(victim)` from `setVictim`, and `updatePath()` from the pulse's interaction check. This generator runs
those calls through the movement oracle's harness (oracle/movement/generate.py and Support.java: the unchanged
MovementPulse, DestinationFlag, Pulse, Point, Direction, Path and Vector with its recording world), with the cases
below and the fixture folder fixtures/attack-chase. Each action that stands for a CombatPulse statement carries a
`chase` key naming it; the native replay runs those through world::combat_chase. The harness ignores the key.
Java/Kotlin are test oracles only; --check never rewrites a fixture, and fixtures/movement is never touched.
"""
import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import pathlib
import random
import shutil

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/attack-chase-oracle'
FIXTURES = ROOT / 'fixtures/attack-chase'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Redirect(type(ROOT)):
    """The repository root as the movement generator sees it: its fixture folder is this oracle's."""

    def __truediv__(self, key):
        if str(key).replace('\\', '/') == 'fixtures/movement':
            return pathlib.Path(FIXTURES)
        return super().__truediv__(key)


def op(name, **kw):
    return dict(op=name, **kw)


# CombatPulse's statements as harness actions.
def new(**kw):
    """`movement = object : MovementPulse(entity, null) { override fun pulse() = false }`."""
    return op('new', target=None, chase='new', **kw)


def victim(target='target'):
    """`movement.setLast(null); movement.setDestination(victim)` (CombatPulse.setVictim)."""
    return op('set', target=target, last=None, chase='setVictim')


def path():
    """`movement.updatePath()` (CombatPulse's interaction check)."""
    return op('path', chase='updatePath')


HOME = [3207, 3216, 0]


def requests():
    out = []
    rng = random.Random(53533)

    def add(actions, mover=None, target=None, other=None, **kw):
        nodes = dict(mover=dict(dict(type='player', location=HOME, victim='target', combat=True), **(mover or {})),
                     target=dict(dict(type='npc', location=[3210, 3216, 0]), **(target or {})))
        if other is not None:
            nodes['other'] = other
        out.append(dict(nodes=nodes, actions=actions, pulseResult=False, **kw))

    chase = [new(), victim(), path()]
    # Every mover and victim kind, size and relative position (beside, diagonal, on top, inside, far, another plane).
    offsets = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (0, 0), (2, 0), (3, 3), (-4, 2), (10, 0), (25, 0), (26, 0), (40, -30)]
    for mover_type in ['player', 'npc', 'entity']:
        for target_type in ['npc', 'player']:
            for size, target_size in [(1, 1), (1, 2), (1, 3), (2, 1), (3, 3)]:
                for dx, dy in offsets:
                    add(chase + [path()], mover=dict(type=mover_type, size=size), target=dict(type=target_type, size=target_size, location=[3207 + dx, 3216 + dy, 0]))
    for plane in [1, 3]:
        add(chase, target=dict(location=[3210, 3216, plane]))
        add(chase, mover=dict(location=[3207, 3216, plane]), target=dict(location=[3210, 3216, plane]))
    # The destination is the combat pulse's victim, or not (the path interrupt of an NPC destination).
    for mover_victim in ['target', None, 'other']:
        for points in [[], [[3210, 3216]], [[3210, 3216], [3211, 3216]], [[3210, 3216], [3211, 3216], [3212, 3217], [3213, 3218]]]:
            for running in [False, True]:
                for permitted in [True, False]:
                    add(chase + [path()], mover=dict(victim=mover_victim, running=running), target=dict(points=points, moving=bool(points)),
                        other=dict(type='npc', location=[3200, 3210, 0]), permitted=permitted)
    # A victim that is itself fighting the mover and walking: the mover waits unless the victim's path is its own tile.
    for combat in [False, True]:
        for target_victim in ['mover', None, 'other']:
            for points in [[], [[3210, 3216]], [[3211, 3216]], [[3210, 3216], [3211, 3216]]]:
                add(chase, target=dict(points=points, combat=combat, victim=target_victim, type='npc'), other=dict(type='player', location=[3200, 3210, 0]))
                add(chase, target=dict(points=points, combat=combat, victim=target_victim, type='player'), other=dict(type='player', location=[3200, 3210, 0]))
    # What the pathfinder answers.
    for successful in [False, True]:
        for near in [False, True]:
            for points in [[], [[3208, 3216]], [[3208, 3216], [3209, 3216]], [[3208, 3217], [3209, 3218], [3210, 3217]]]:
                add(chase + [path(), op('node', node='target', location=[3212, 3218, 0]), path()], paths=[dict(successful=successful, near=near, points=points)])
    add(chase, paths=[dict(null=True)])
    add(chase + [path()], paths=[dict(points=[[3208, 3216]]), dict(null=True)])
    # The same destination twice while the mover still has a path, then the victim moves, then another victim.
    add(chase + [path(), path(), op('node', node='target', location=[3214, 3220, 0]), path(), path(), victim('other'), path(), path(), victim('target'), path()],
        other=dict(type='player', location=[3200, 3210, 0]))
    add(chase + [op('pointClear'), path(), op('node', node='mover', location=[3209, 3216, 0]), path(), op('node', node='mover', location=[3210, 3216, 0]), path()])
    add([new(), path(), victim(), path(), victim(None), path(), victim(), path()])
    add([new(), victim(), op('node', node='target', location=None), path(), op('node', node='target', location=[3209, 3217, 0]), path()])
    # The combat pulse is made for an entity that already walks somewhere, and made again.
    add([op('new', target='other'), op('path'), new(), victim(), path(), new(), victim(), path()], other=dict(type='location', location=[3215, 3216, 0]))
    add([new(), victim(), path(), op('new', target='other', constructor='flag', flag='location'), op('path')], other=dict(type='location', location=[3215, 3216, 0]))
    # NPC movers: never-walking, a behaviour's pathfinder, no behaviour.
    for fields in [dict(never=True), dict(finder=2), dict(finder=0), dict(behavior=False), dict(), dict(never=True, finder=2)]:
        add(chase + [path()], mover=dict(type='npc', **fields), target=dict(type='player'))
    # A player's draw-intersect debug attribute.
    for draw in [False, True]:
        for dx in [1, 3, 30]:
            add(chase + [path()], mover=dict(draw=draw), target=dict(location=[3207 + dx, 3216, 0]))
    # Inside the victim: the border location, with blocked tiles around.
    for size in [2, 3, 5]:
        for dx, dy in [(0, 0), (1, 1), (size - 1, 0), (0, size - 1)]:
            for clip in [0, 0x80, 0x20, 0x2, 0x8, -1]:
                add(chase, mover=dict(location=[3210 + dx, 3216 + dy, 0]), target=dict(size=size), clip=clip)
            add(chase, mover=dict(location=[3210 + dx, 3216 + dy, 0]), target=dict(size=size, moving=True, points=[[3210, 3216]]))
    # The pulse itself, were it ever updated (CombatPulse never submits it): `pulse()` answers false.
    for region in [None, False, True]:
        add(chase + [op('update'), op('update'), op('stop'), op('update')], region=region)
    add(chase + [op('update'), op('node', node='mover', location=[3209, 3216, 0]), op('update'), op('update')])
    add(chase + [op('update')], mover=dict(location=[3209, 3216, 0]))
    # Every service fails, moves a node or drops the victim once.
    for event in ['clip', 'permitted', 'find', 'reset', 'add', 'face', 'hintClear', 'hintAdd', 'npcFinder']:
        for hook in [op('fail'), op('set', target=None), op('node', node='target', location=[3211, 3218, 1]), op('node', node='mover', location=[3205, 3214, 0]), op('set', running=False)]:
            for mover_type in ['player', 'npc']:
                add(chase + [path(), victim(), path()], mover=dict(type=mover_type, draw=True, location=[3211, 3217, 0] if event in ('clip',) else HOME),
                    target=dict(size=3 if event == 'clip' else 1, points=[[3210, 3216], [3211, 3216]] if event == 'permitted' else []),
                    hooks=[dict(event=event, actions=[hook])])
    for event in ['permitted', 'clip']:
        for at in [1, 2, 3]:
            add(chase + [path()], mover=dict(victim=None, location=[3211, 3217, 0] if event == 'clip' else HOME),
                target=dict(size=3 if event == 'clip' else 1, points=[[3210, 3216], [3211, 3216], [3212, 3216]] if event == 'permitted' else []),
                hooks=[dict(event=event, at=at, actions=[op('fail')])])
    # A random walk.
    for _ in range(80):
        mover = dict(type=rng.choice(['player', 'npc']), location=[3207, 3216, rng.randrange(2)], size=rng.randrange(1, 3), running=bool(rng.randrange(2)),
                     victim=rng.choice(['target', 'target', None, 'other']), draw=bool(rng.randrange(4) == 0))
        target = dict(type=rng.choice(['npc', 'player']), location=[3207 + rng.randrange(-5, 30), 3216 + rng.randrange(-5, 6), rng.randrange(2)], size=rng.randrange(1, 4),
                      moving=bool(rng.randrange(2)), points=rng.choice([[], [[3210, 3216]], [[3210, 3216], [3211, 3217]]]), combat=bool(rng.randrange(2)), victim=rng.choice(['mover', None]))
        other = dict(type=rng.choice(['npc', 'player']), location=[3207 + rng.randrange(-8, 9), 3216 + rng.randrange(-8, 9), 0], size=rng.randrange(1, 3))
        actions = [new(), victim()]
        for _ in range(rng.randrange(2, 9)):
            kind = rng.choice(['path', 'path', 'path', 'victim', 'moveTarget', 'moveMover', 'points', 'new'])
            if kind == 'path':
                actions.append(path())
            elif kind == 'victim':
                actions.append(victim(rng.choice(['target', 'other', None])))
            elif kind == 'moveTarget':
                actions.append(op('node', node=rng.choice(['target', 'other']), location=[3207 + rng.randrange(-5, 30), 3216 + rng.randrange(-5, 6), rng.randrange(2)]))
            elif kind == 'moveMover':
                actions.append(op('node', node='mover', location=[3207 + rng.randrange(-3, 4), 3216 + rng.randrange(-3, 4), rng.randrange(2)]))
            elif kind == 'points':
                actions.append(op('pointClear'))
            else:
                actions += [new(), victim(rng.choice(['target', 'other']))]
        paths = [rng.choice([dict(), dict(successful=False), dict(near=True), dict(points=[]), dict(points=[[3208, 3216], [3209, 3217]])]) for _ in range(10)]
        add(actions, mover=mover, target=target, other=other, paths=paths, clip=rng.choice([0, 0, 0, 0x80, 0x2]), permitted=bool(rng.randrange(5)))
    return out


def summary(rows):
    statements, events, errors = {}, {}, {}
    steps = 0
    for row in rows:
        for action, step in zip(row['actions'], row['expected']):
            steps += 1
            if 'chase' in action:
                statements[action['chase']] = statements.get(action['chase'], 0) + 1
            if step.get('error'):
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    return dict(cases=len(rows), actions=steps, statements=dict(sorted(statements.items())), recordedCalls=sum(events.values()),
                events=dict(sorted(events.items())), errors=dict(sorted(errors.items())))


def generate(check, java_home):
    RUN.mkdir(parents=True, exist_ok=True)
    name = 'json-simple-1.1.1.jar'
    pin = json.loads((ROOT / 'oracle/auth/json.lock.json').read_text(encoding='utf-8'))['sha256']
    candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
    jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
    if jar is None:
        raise RuntimeError(f'{name} matching its pin was not found in runs/*-oracle (this generator never downloads)')
    if jar != RUN / name:
        shutil.copyfile(jar, RUN / name)
    for stale in ('classes', 'adapters'):
        if (RUN / stale).exists():
            shutil.rmtree(RUN / stale)
    movement = load('movement_generator', ROOT / 'oracle/movement/generate.py')
    movement.ROOT = Redirect(ROOT)
    movement.RUN = RUN
    movement.requests = requests
    movement.c.JAVA_HOME = java_home
    printed = io.StringIO()
    with contextlib.redirect_stdout(printed):
        movement.generate(check)
    data = (FIXTURES / 'oracle.jsonl').read_bytes()
    rows = [json.loads(line) for line in data.decode('utf-8').splitlines() if line]
    mistakes = sorted({step['error'] for row in rows for step in row['expected'] if step.get('error') not in (None, 'Null', 'Binding')})
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus mistakes): {mistakes}')
    generator = hashlib.sha256(pathlib.Path(__file__).read_bytes().replace(b'\r', b'')).hexdigest()
    chase = dict(format=1, generatorSha256=generator, fixtureSha256=hashlib.sha256(data).hexdigest(), coverage=summary(rows), sourceHashAlgorithm='sha256, CR removed',
                 harness='oracle/movement/generate.py and oracle/movement/Support.java (pinned in provenance.json)',
                 scope='The unchanged MovementPulse as CombatPulse configures it: MovementPulse(entity, null) (action `new` with a null target), '
                       'setLast(null) and setDestination(victim) (action `set`), updatePath() (action `path`); the anonymous subclass answers '
                       'pulse() with false (pulseResult). CombatPulse.attack, setVictim and the interaction check themselves are '
                       'combat-pulse-oracle; the pathfinders and the walking queue are their own corpora.')
    text = f'{len(rows)} original combat chase cases ({chase["coverage"]["actions"]} actions, {chase["coverage"]["recordedCalls"]} recorded calls)'
    if check:
        if json.loads((FIXTURES / 'chase.json').read_text(encoding='utf-8')) != chase:
            raise RuntimeError('attack-chase chase.json differs; inspect before regeneration')
        print(f'{text} reproduced.')
    else:
        (FIXTURES / 'chase.json').write_text(json.dumps(chase, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {text}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    home = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check, home)
