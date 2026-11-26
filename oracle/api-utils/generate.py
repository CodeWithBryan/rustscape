"""Run the original core/api/utils WeightedTable, PlayerCamera and CameraUtils on JDK 11 and record what they do.

Compiled unchanged: core/api/utils/WeightedTable.kt, PlayerCamera.kt and CameraUtils.kt, core/net/packet/Context.java
and core/net/packet/context/CameraContext.java. Original text inserted into an adapter at generation time:
RandomFunction.random(double, double). PacketRepository.send is a recording adapter. The two generators
WeightedTable.roll draws from (the JDK's shared Collections.shuffle generator, set by reflection, and
RandomFunction.RANDOM) are seeded by the case and record their draws. Each case runs in a class loader of its own
(oracle/content-api/Boot.java). Java/Kotlin are test oracles only; --check never rewrites a fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import random
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/api-utils-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/api-utils'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')

UNCHANGED = [
    'core/api/utils/WeightedTable.kt',
    'core/api/utils/PlayerCamera.kt',
    'core/api/utils/CameraUtils.kt',
    'core/net/packet/Context.java',
    'core/net/packet/context/CameraContext.java',
]
SCOPE = (
    'WeightedTable.kt, PlayerCamera.kt, CameraUtils.kt, Context.java and CameraContext.java compiled unchanged on JDK 11 '
    'with Kotlin 1.8.20; RandomFunction.random(double, double) inserted verbatim. PacketRepository.send is a recording '
    'adapter; Collections.shuffle\'s shared generator and RandomFunction.RANDOM are java.util.Random objects seeded by '
    'the case that record nextInt(bound) and nextDouble(). Every recorded call in order, results, errors, the table '
    'entries and total weight (as bits) and the camera context after each operation, with injected failures of every '
    'recorded call. WeightBasedTable and WeightedItem are drop-tables-oracle; Vector is content-api-oracle; '
    'PlayerStatsCounter and Permadeath are not covered.'
)

ADAPTERS = {
    'core/game/node/entity/player/Player.java': 'package core.game.node.entity.player;\npublic class Player {}\n',
    'core/net/packet/PacketRepository.java': r'''package core.net.packet;
public class PacketRepository {
    public static void send(Class<?> packet, Context context) { oracle.Support.sent(packet, context); }
}
''',
    'core/net/packet/out/CameraViewPacket.java': 'package core.net.packet.out;\npublic class CameraViewPacket {}\n',
    'core/tools/RandomFunction.java': r'''package core.tools;
import java.util.Random;
public class RandomFunction {
    public static Random RANDOM = new Random();
    public static void oracleSeed(long seed) { RANDOM = new oracle.Support.Recording("RandomFunction.RANDOM", seed); }
    /* ORIGINAL_RANDOM_DOUBLE */
}
''',
}


def markers():
    tools = c.read(SERVER / 'core/tools/RandomFunction.java')
    return {'ORIGINAL_RANDOM_DOUBLE': c.method(tools, 'public static final double random(double a, double b) {')}


def libraries():
    pin = json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))['sha256']
    name = 'json-simple-1.1.1.jar'
    candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
    jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
    if jar is None:
        raise RuntimeError(f'{name} matching its pin was not found in runs/*-oracle (this generator never downloads)')
    if jar != RUN / name:
        shutil.copyfile(jar, RUN / name)
    return [RUN / name]


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    reference = ROOT / 'reference' / 'server'
    if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock['server']['commit']:
        raise RuntimeError('Reference pin differs')
    c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    annotations = next(path for path in jars if path.name.startswith('annotations-'))
    libs = [stdlib, annotations, *libraries()]
    kotlin, java = [], []
    for name in UNCHANGED:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(ADAPTERS.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    c.read(HERE / 'Calls.kt')
    kotlin.append(HERE / 'Calls.kt')
    boot_source = ROOT / 'oracle/content-api/Boot.java'
    c.read(boot_source)
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    boot = RUN / 'boot'
    if boot.exists():
        shutil.rmtree(boot)
    boot.mkdir()
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-d', boot, boot_source])
    return lock, os.pathsep.join(map(str, [boot, *libs])), classes


def execute(classpath, classes, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    # Collections.shuffle's shared generator is a private static of java.util.Collections.
    result = subprocess.run([str(executable), '-Xmx1g', '--add-opens', 'java.base/java.util=ALL-UNNAMED', '-Dfile.encoding=UTF-8', '-Duser.language=en',
                             '-Duser.country=US', '-cp', classpath, 'oracle.boot.Boot', str(classes)], input=payload, cwd=ROOT, capture_output=True, timeout=1800)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


# ------------------------------------------------------------------ cases
SEEDS = [(1, 2), (530, 2009), (-7, 0), (0x5DEECE66D, -1), (123456789012345, 987654321)]


def case(name, ops, sweep=False, seeds=(1, 2)):
    return dict(name=name, ops=ops, shuffleSeed=seeds[0], randomSeed=seeds[1]), sweep


def cases():
    out = []
    names = ['baby', 'young', 'gourmet', 'earth', 'essence', 'eclectic', 'nature', 'magpie', 'ninja', 'dragon']
    # roll over tables of every small size, with each seed pair.
    for size in [0, 1, 2, 3, 4, 5, 6, 8, 10]:
        for s, seeds in enumerate(SEEDS):
            entries = [[names[i], float(10 - i)] for i in range(size)]
            out.append(case(f'roll/size{size}/seed{s}', [['create', entries]] + [['roll']] * 6, sweep=size in (2, 5) and s == 0, seeds=seeds))
    # Weights: equal, zero, negative, fractional, huge, tiny, infinite, not a number, signed zero.
    weight_sets = [[1.0, 1.0, 1.0], [0.0, 0.0, 0.0], [-1.0, -2.0, -3.0], [5.0, -5.0, 1.0], [0.1, 0.2, 0.7], [1e300, 1e300, 1e300], [1e-300, 5e-324, 0.0],
                   ['Infinity', 1.0, 2.0], ['-Infinity', 1.0, 2.0], ['NaN', 1.0, 2.0], [1.0, 'NaN', 2.0], ['-0.0', 0.0, '-0.0'], ['Infinity', '-Infinity', 1.0],
                   [100.0, 0.0, 0.0], [0.0, 0.0, 100.0], [1.0, 1e-17, 1.0]]
    for w, weights in enumerate(weight_sets):
        for s, seeds in enumerate(SEEDS[:3]):
            entries = [[names[i], weight] for i, weight in enumerate(weights)]
            out.append(case(f'weights/{w}/seed{s}', [['create', entries]] + [['roll']] * 5, seeds=seeds))
            out.append(case(f'weights/{w}/added/seed{s}', [['add', names[i], weight] for i, weight in enumerate(weights)] + [['roll']] * 3, seeds=seeds))
    # Elements: null, ints, duplicates.
    out.append(case('elements/null', [['create', [[None, 1.0], ['a', 1.0], [None, 3.0]]]] + [['roll']] * 8 + [['remove', None], ['roll'], ['remove', None], ['roll'], ['remove', None], ['roll']], sweep=True))
    out.append(case('elements/ints', [['create', [[1024, 20.0], [1025, 10.0], [1026, 1.0], [-1, 0.5]]]] + [['roll']] * 8 + [['remove', 1025], ['remove', 1025], ['remove', '1024'], ['roll'], ['roll']]))
    out.append(case('elements/duplicates', [['add', 'a', 1.0], ['add', 'a', 2.0], ['add', 'b', 4.0], ['remove', 'a'], ['roll'], ['roll'], ['remove', 'a'], ['roll'], ['remove', 'a'], ['remove', 'b'], ['roll'], ['remove', 'b']]))
    # removeAt: every index and the bounds; the total follows each removal.
    for index in [-1, 0, 1, 2, 3, 2147483647, -2147483648]:
        out.append(case(f'removeAt/{index}', [['create', [['a', 1.5], ['b', 2.5], ['c', 4.0]]], ['removeAt', index], ['roll'], ['roll'], ['removeAt', 0], ['removeAt', 0], ['removeAt', 0], ['roll']]))
    # ArrayList members the class inherits: a raw add and clear leave the total as it was.
    out.append(case('inherited/raw-add', [['add', 'a', 1.0], ['addRaw', 'b', 100.0], ['roll'], ['roll'], ['roll'], ['roll'], ['remove', 'b'], ['roll'], ['add', 'c', 1.0], ['roll'], ['roll']], sweep=True))
    out.append(case('inherited/clear', [['create', [['a', 1.0], ['b', 2.0]]], ['clear'], ['roll'], ['add', 'c', 1.0], ['roll'], ['add', 'd', 1.0], ['roll'], ['roll'], ['roll'], ['roll']]))
    out.append(case('inherited/total-set', [['create', [['a', 1.0], ['b', 2.0], ['c', 3.0]]], ['total', 0.5], ['roll'], ['roll'], ['total', 1000.0], ['roll'], ['roll'], ['total', -6.0], ['roll'], ['total', 'NaN'], ['roll']]))
    out.append(case('create/empty', [['create', []], ['roll'], ['remove', 'a'], ['removeAt', 0], ['add', 'a', 0.0], ['roll'], ['add', 'b', 0.0], ['roll']]))
    # A random walk over the table.
    rng = random.Random(53534)
    for n in range(60):
        ops = []
        for _ in range(rng.randrange(6, 20)):
            kind = rng.choice(['add', 'add', 'add', 'roll', 'roll', 'roll', 'remove', 'removeAt', 'addRaw', 'clear', 'create'])
            element = rng.choice(names[:5] + [None, 7, 8])
            weight = rng.choice([0.0, 0.5, 1.0, 2.0, 3.25, 10.0, -1.0, 1e-9])
            if kind in ('add', 'addRaw'):
                ops.append([kind, element, weight])
            elif kind == 'remove':
                ops.append(['remove', element])
            elif kind == 'removeAt':
                ops.append(['removeAt', rng.randrange(-1, 5)])
            elif kind == 'create':
                ops.append(['create', [[rng.choice(names[:5]), rng.choice([0.5, 1.0, 4.0])] for _ in range(rng.randrange(0, 7))]])
            elif kind == 'clear' and rng.randrange(3) == 0:
                ops.append(['clear'])
            else:
                ops.append(['roll'])
        out.append(case(f'table/random/{n}', ops, sweep=n < 6, seeds=(rng.randrange(-2**40, 2**40), rng.randrange(-2**40, 2**40))))
    # PlayerCamera: every member with and without a player, rotateBy from every kind of context.
    moves = [['setPosition', 51, 49, 300], ['rotateTo', 47, 52, 250, 100], ['panTo', 40, 41, 500, 10], ['shake', 4, 2, 10, 128, 2], ['reset']]
    for player in [True, False]:
        out.append(case(f'camera/all/{player}', [['camera', player]] + moves + [['rotateBy', 1, -1, 5, 7]], sweep=player))
        for m, move in enumerate(moves):
            out.append(case(f'camera/rotateBy/{player}/{m}', [['camera', player], ['rotateBy', 3, 4, 5, 6], move, ['rotateBy', 3, 4, 5, 6], ['rotateBy', -10, -20, -30, 0], ['forget'], ['rotateBy', 1, 1, 1, 1]],
                            sweep=player and m in (0, 4)))
    out.append(case('camera/wrapping', [['camera', True], ['setPosition', 2147483647, -2147483648, 2147483647], ['rotateBy', 1, -1, 1, -5], ['rotateBy', 2147483647, 2147483647, 2147483647, 2147483647],
                                        ['shake', -1, 256, 65536, -2147483648, 2147483647], ['rotateBy', 0, 0, 0, 0]]))
    out.append(case('camera/enums', [['shakeTypes'], ['cameraTypes']]))
    out.append(case('camera/none', [['setPosition', 1, 2, 3]]))
    return out


EXPECTED_ERRORS = (None, 'Null', 'Binding', 'Index')


def generate(check):
    lock, classpath, classes = build()
    planned = cases()
    rows = execute(classpath, classes, [x for x, _ in planned])
    failing = []
    for (row_case, sweep), row in zip(planned, rows):
        if not sweep:
            continue
        for at in range(1, row['expected']['calls'] + 1):
            failing.append(dict(row_case, failAt=at, name=row_case['name'] + f'/fail{at}'))
    rows += execute(classpath, classes, failing)
    mistakes = {}
    for row in rows:
        for step in row['expected']['steps']:
            error = step.get('error')
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error, row['name'])
            if error == 'Binding' and 'failAt' not in row:
                mistakes.setdefault(error, row['name'])
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, errors, events = {}, {}, {}
    steps = 0
    for row in rows:
        for op, step in zip(row['ops'], row['expected']['steps']):
            steps += 1
            operations[op[0]] = operations.get(op[0], 0) + 1
            if step.get('error'):
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(cases=len(rows), operations=dict(sorted(operations.items())), steps=steps, recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), failureCases=len(failing))
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows),
                      fixtureSha256=hashlib.sha256(data).hexdigest(), coverage=coverage, sourceHashAlgorithm='sha256, CR removed',
                      kotlinCompiler='1.8.20', unchanged=UNCHANGED, scope=SCOPE)
    summary = f'{len(rows)} original core/api/utils cases ({steps} operations, {coverage["recordedCalls"]} recorded calls, {len(failing)} failure cases)'
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('api-utils fixtures/provenance differ; inspect before regeneration')
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
