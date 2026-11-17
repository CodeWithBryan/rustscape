"""Run the unchanged zone classes and Region.java on JDK 11 and record what they do.

Compiled unchanged: ZoneBorders, Zone, ZoneType, ZoneRestriction, RegionZone,
ZoneBuilder, MapZone, ZoneMonitor, the BankZone, KaramjaZone, ModeratorZone and
MultiwayCombatZone implementations, MusicZone, Region.java, MapDistance,
Pulse.java, StartupListener and ContentInterface. PulseRunner.kt is compiled
with one added import alias for its clock. Everything else is an adapter from
adapters.py, with the original text of the methods named in `markers` inserted
at generation time. --check never updates a fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/zones-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/zones'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('zones_adapters', HERE / 'adapters.py')
corpus = load('zones_cases', HERE / 'cases.py')

ORIGINAL = [
    'core/game/world/map/zone/ZoneBorders.java',
    'core/game/world/map/zone/Zone.java',
    'core/game/world/map/zone/ZoneType.java',
    'core/game/world/map/zone/ZoneRestriction.java',
    'core/game/world/map/zone/RegionZone.java',
    'core/game/world/map/zone/ZoneBuilder.java',
    'core/game/world/map/zone/MapZone.java',
    'core/game/world/map/zone/ZoneMonitor.java',
    'core/game/world/map/zone/impl/BankZone.java',
    'core/game/world/map/zone/impl/KaramjaZone.java',
    'core/game/world/map/zone/impl/ModeratorZone.java',
    'core/game/world/map/zone/impl/MultiwayCombatZone.java',
    'core/game/node/entity/player/link/music/MusicZone.java',
    'core/game/world/map/Region.java',
    'core/game/world/map/MapDistance.java',
    'core/game/system/task/Pulse.java',
    'core/api/StartupListener.kt',
    'core/api/ContentInterface.kt',
]
SCOPE = (
    'Unchanged ZoneBorders, Zone, ZoneType, ZoneRestriction, RegionZone, ZoneBuilder, MapZone, ZoneMonitor, BankZone, KaramjaZone, '
    'ModeratorZone, MultiwayCombatZone, MusicZone, Region.java, MapDistance and Pulse on JDK 11; PulseRunner with one added import alias '
    'for its clock. Original text inserted at generation time: RegionManager.forId over its cache map and lock, RegionChunk.clear, '
    'Entity.isTeleBlocked, Pathfinder.isStandingIn, RandomFunction.RANDOM and normalRandDist(max, intensity), the Log enum, and '
    'Location\'s packed hash, region id forms, transform, toString, equals and hashCode. Everything else is a recording adapter: '
    'Entity, Player, NPC, Familiar and all their sub-objects, Item, Option, CombatStyle, RequestType, Rights, TeleportManager, the '
    'packet classes, the local entity queries and isTeleportPermitted of RegionManager, the rest of RegionChunk, Cache (no map '
    'archive is ever found, so Region.load stops after its lookup), DynamicRegion, the map parsers, Scenery, GameWorld, Repository, '
    'the log function, ServerConstants, and placeholder WildernessZone and DarkZone classes that exist only so ZoneBuilder compiles. '
    'MapZone.checkMulti reads the real wall clock: cases use combat times that are far from it. getRandomLoc draws from unseeded '
    'generators: only facts that do not depend on the draw are recorded. Not exercised: ZoneBuilder.startup, MapZone.cleanItems, '
    'Region.load beyond the archive lookup, the assemble lists, dynamic regions, self-referencing exception lists and region id '
    'lists beyond a few regions.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def markers():
    read = lambda name: c.read(SERVER / name)
    location = read('core/game/world/map/Location.kt')
    functions = read('core/tools/RandomFunction.java')
    members = [
        line(location, 'override fun toString()'),
        line(location, 'override fun hashCode()'),
        c.method(location, 'override fun equals(other: Any?): Boolean'),
        line(location, 'val region: Region'),
        line(location, 'val regionId: Int'),
        line(location, 'val regionX: Int'),
        line(location, 'val regionY: Int'),
        line(location, 'fun transform(diffX: Int, diffY: Int, z: Int)'),
    ]
    return {
        'ORIGINAL_IS_TELE_BLOCKED': c.method(read('core/game/node/entity/Entity.java'), 'public boolean isTeleBlocked()'),
        'ORIGINAL_IS_STANDING_IN': c.method(read('core/game/world/map/path/Pathfinder.java'), 'public static boolean isStandingIn('),
        'ORIGINAL_RANDOM_FIELD': line(functions, 'public static final Random RANDOM'),
        'ORIGINAL_NORMAL_RAND_DIST': c.method(functions, 'public static int normalRandDist(int max, int intensity)'),
        'ORIGINAL_LOCATION_MEMBERS': '\n    '.join(members),
        'ORIGINAL_LOCATION_HASH': c.method(location, 'value class LocationHash('),
        'ORIGINAL_FOR_ID': c.method(read('core/game/world/map/RegionManager.kt'), 'fun forId(regionId: Int): Region'),
        'ORIGINAL_CHUNK_CLEAR': c.method(read('core/game/world/map/RegionChunk.java'), 'public void clear()'),
        'ORIGINAL_LOG_ENUM': c.method(read('core/tools/SystemLogger.kt'), 'enum class Log {'),
    }


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)

    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libraries = [stdlib]
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))
        jar = RUN / name
        # Pinned by hash; this generator never downloads. Place the artifact here from another oracle's run directory.
        if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != pin['sha256']:
            raise RuntimeError(f'{jar} is missing or differs from its pin')
        libraries.append(jar)

    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    texts = dict(adapters.ADAPTERS)
    runner = c.read(SERVER / 'core/game/world/PulseRunner.kt')
    texts['core/game/world/PulseRunner.kt'] = runner.replace('package core.game.world', 'package core.game.world\nimport core.game.world.PulseClock as System', 1)
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(texts.items()):
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
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')

    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx768m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, libraries)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])

    cases = corpus.requests()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath, 'oracle.Support']
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=600)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')

    events, errors, operations = {}, {}, {}
    for row in rows:
        for action, step in zip(row['actions'], row['expected']['results']):
            operations[action['op']] = operations.get(action['op'], 0) + 1
            if 'error' in step:
                errors[step['error']] = errors.get(step['error'], 0) + 1
        for event in row['expected']['events']:
            name = event[0].split('.')[-1] if '.' in event[0] else event[0]
            events[name] = events.get(name, 0) + 1
    coverage = dict(
        actions=sum(operations.values()),
        operations=dict(sorted(operations.items())),
        recordedCalls=sum(events.values()),
        events=dict(sorted(events.items())),
        errors=dict(sorted(errors.items())),
        groups=corpus.groups(cases),
    )
    for needed in ['enter', 'leave', 'locationUpdate', 'entityUpdate', 'unlock', 'playDefault', 'setInvisible', 'packetMessage', 'playerMessage',
                   'archiveId', 'renderable', 'regionInactivity', 'log', 'stackTrace', 'teleportPermitted', 'packet', 'teleporterSend',
                   'inventoryRemove', 'restoreTabs', 'localNpcs', 'localPlayers']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    for needed in ['Failure', 'Null', 'ClassCast', 'ConcurrentModification', 'IllegalArgument', 'Arithmetic', 'IllegalState']:
        if needed not in errors:
            raise RuntimeError(f'the corpus never fails with {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', scope=SCOPE)
    summary = (f'{len(rows)} original zone and region activity cases ({coverage["actions"]} actions, '
               f'{coverage["recordedCalls"]} recorded calls, {sum(errors.values())} failures)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Zone fixtures/provenance differ; inspect before regeneration')
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
