"""Run the unchanged ForceMovement on JDK 11 and record what it does.

Compiled unchanged: core/game/node/entity/impl/ForceMovement.java, core/game/system/task/Pulse.java,
core/game/world/map/Direction.java, Point.java, path/ClipMaskSupplier.java,
core/game/world/update/flag/context/Animation.java and ForceMoveCtx.kt, and core/game/world/update/flag/EntityFlag.kt.
The members named in `markers` are original text inserted into the adapters of adapters.py at generation time.
Everything else is a recording adapter answering from the case. Java/Kotlin are test oracles only; --check never
rewrites a fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import re
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/force-movement-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/force-movement'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('force_movement_adapters', HERE / 'adapters.py')
corpus = load('force_movement_cases', HERE / 'cases.py')

UNCHANGED = [
    'core/game/node/entity/impl/ForceMovement.java',
    'core/game/system/task/Pulse.java',
    'core/game/world/map/Direction.java',
    'core/game/world/map/Point.java',
    'core/game/world/map/path/ClipMaskSupplier.java',
    'core/game/world/update/flag/context/Animation.java',
    'core/game/world/update/flag/context/ForceMoveCtx.kt',
    'core/game/world/update/flag/EntityFlag.kt',
]
SCOPE = (
    'Unchanged ForceMovement.java with Pulse.java, Direction.java, Point.java, ClipMaskSupplier.java, Animation.java, '
    'ForceMoveCtx.kt and EntityFlag.kt on JDK 11 with Kotlin 1.8.20 and JSON-simple 1.1.1. Original text inserted into '
    'adapters at generation time: Location\'s LocationHash packing, constructors, x/y/z properties, create, getDistance '
    '(both) and getDelta; Animator.Priority. Everything else is a recording adapter answering from the case: the entity '
    'location, animate (with the animation object and its priority at the call), setDisabledTicks, the update mask '
    'registration with the ForceMoveCtx fields, WalkingQueue.updateRegion, setTeleportLocation, lock, unlock, '
    'GameWorld.Pulser.submit and the Pulse node check (isActive). Printed stack traces are recorded by exception class. '
    'WALK_ANIMATION is one static object; each case starts from its initialiser\'s priority (MID). Not driven: '
    'Direction.canMove/RegionManager and Animation.getDuration.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def between(text, start, end):
    a = text.index(start)
    b = text.index(end, a)
    return text[a:b].rstrip()


def markers():
    read = lambda name: c.read(SERVER / name)
    location = read('core/game/world/map/Location.kt')
    animator = read('core/game/node/entity/impl/Animator.java')
    hash_start = location.index('    @JvmInline\n    value class LocationHash')
    return {
        'ORIGINAL_LOCATION_CONSTRUCTORS': between(location, '    @JvmOverloads\n    constructor(x: Int, y: Int, z: Int = 0)', '    constructor(location: Location)'),
        'ORIGINAL_LOCATION_PROPERTIES': between(location, '    var x: Int', '    companion object {'),
        'ORIGINAL_LOCATION_CREATE': line(location, '@JvmStatic @JvmOverloads fun create(x: Int, y: Int, z: Int = 0): Location'),
        'ORIGINAL_LOCATION_DISTANCE': c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location): Double'),
        'ORIGINAL_LOCATION_DELTA': line(location, '@JvmStatic fun getDelta(location: Location, other: Location): Location'),
        'ORIGINAL_LOCATION_INSTANCE_DISTANCE': line(location, 'fun getDistance(other: Location): Double = getDistance(this, other)'),
        'ORIGINAL_LOCATION_HASH': c.method(location[hash_start:], '@JvmInline\n    value class LocationHash(val hash: Int) {'),
        'ORIGINAL_PRIORITY': c.method(animator, 'public static enum Priority'),
    }


def libraries():
    pin = json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))['sha256']
    name = 'json-simple-1.1.1.jar'
    candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
    jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
    if jar is None:
        # Pinned by hash; this generator never downloads.
        raise RuntimeError(f'{name} matching its pin was not found in runs/force-movement-oracle or another runs/*-oracle directory')
    if jar != RUN / name:
        shutil.copyfile(jar, RUN / name)
    return [RUN / name]


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    runtime = [p for p in jars if p.name.startswith('kotlin-stdlib-')]
    libs = [*runtime, *libraries()]
    kotlin, java = [], []
    for name in UNCHANGED:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(adapters.ADAPTERS.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'adapters.py')
    c.read(HERE / 'cases.py')
    c.read(HERE / 'Support.kt')
    kotlin.append(HERE / 'Support.kt')
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    return lock, classpath


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Support'], input=payload, cwd=ROOT, capture_output=True, timeout=1800)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Binding')


def generate(check):
    lock, classpath = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, cases)
    # Second pass: in each marked operation every recorded call fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        calls = len([e for e in row['expected'][sweep]['events'] if e[0] != 'exception'])
        for at in range(1, calls + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    rows += execute(classpath, failing)
    cases += failing
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['input']['ops'], row['expected']):
            if step['error'] not in EXPECTED_ERRORS or (step['error'] == 'Binding' and 'failAt' not in op):
                mistakes.setdefault(step['error'], (number, op))
            for event in step['events']:
                if event[0] == 'exception' and event[1] not in EXPECTED_ERRORS:
                    mistakes.setdefault(event[1], (number, op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for op, step in zip(row['input']['ops'], row['expected']):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            if step['error']:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), groups=corpus.groups(cases),
                    failureCases=len(failing))
    for needed in corpus.NEEDED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED, scope=SCOPE)
    summary = (f'{len(rows)} original force movement cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('force movement fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
