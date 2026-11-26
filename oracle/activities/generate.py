"""Run the unchanged activity and cutscene framework on JDK 11 and record what it does.

Compiled unchanged: ActivityManager, ActivityPlugin, CutscenePlugin and Cutscene.kt, with MapZone, Zone,
ZoneRestriction, ZoneType, RegionZone, Direction, FacialExpression, CameraShakeType, EventHook/Event and the
whole dynamic-regions set (DynamicRegion, Region, RegionChunk, RegionManager, ChunkFlags, the parsers, Scenery,
SceneryBuilder, SceneryDefinition, the MinimapState writer and its context, IoBuffer...). Original text inserted at
generation time: the dynamic-regions markers (Player.updateSceneGraph, XteaParser's lookup, the build-area
constants, isPlusN, Location's members) plus Location.transform(Direction), ContentAPI's getAttribute,
setAttribute, removeAttribute, openInterface, closeInterface, Player.getCutscene and Player.getCutsceneStage.
Everything else is a recording adapter (adapters.py); the content activities and cutscenes are scripted oracle
content calling only inherited original members.

The cache input is the committed fixtures/dynamic-regions/cache.json (real pinned cache regions, scenery
definitions and XTEA keys exported by `cargo xtask dynamic-regions-oracle`); its hash must match that fixture's
provenance. Every case runs in a class loader of its own (oracle/global-items2/Boot.java). --check never rewrites a
fixture.
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
RUN = ROOT / 'runs/activities-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/activities'
CACHE = ROOT / 'fixtures/dynamic-regions/cache.json'
BOOT = ROOT / 'oracle/global-items2/Boot.java'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
regions = load('activities_dynamic_regions', ROOT / 'oracle/dynamic-regions/generate.py')
adapters = load('activities_adapters', HERE / 'adapters.py')
corpus = load('activities_cases', HERE / 'cases.py')

ORIGINAL = [
    'core/game/activity/ActivityManager.java',
    'core/game/activity/ActivityPlugin.java',
    'core/game/activity/CutscenePlugin.java',
    'core/game/activity/Cutscene.kt',
    'core/game/world/map/zone/MapZone.java',
    'core/game/world/map/zone/Zone.java',
    'core/game/world/map/zone/ZoneRestriction.java',
    'core/game/world/map/zone/ZoneType.java',
    'core/game/world/map/zone/RegionZone.java',
    'core/game/dialogue/FacialExpression.java',
    'core/api/utils/CameraUtils.kt',
    'core/game/event/EventHook.kt',
    'core/game/event/Event.kt',
    'core/plugin/Plugin.java',
    'core/plugin/PluginManifest.java',
    'core/plugin/PluginType.java',
] + [name for name in regions.ORIGINAL if name not in ('core/game/world/map/zone/ZoneBorders.java',)] + ['core/game/world/map/zone/ZoneBorders.java']
SCOPE = (
    'Unchanged ActivityManager, ActivityPlugin, CutscenePlugin and Cutscene.kt on JDK 11 with Kotlin 1.8.20 over the unchanged '
    'MapZone, RegionZone, ZoneRestriction, ZoneType, DynamicRegion, Region, RegionChunk, RegionManager and the MinimapState writer, '
    'with the real pinned cache regions, scenery definitions and XTEA keys of fixtures/dynamic-regions/cache.json. Original text '
    'inserted: the dynamic-regions markers, Location.transform(Direction), ContentAPI getAttribute/setAttribute/removeAttribute/'
    'openInterface/closeInterface, Player.getCutscene and getCutsceneStage. Recording adapters: Player and NPC members (attributes, '
    'properties, locks, walking queue, interface manager, packet dispatch, dialogue interpreter actions, logout listeners, event '
    'hooks, pulse manager), the multiway zone object, GameWorld (settings and a fixed-order pulser), ServerConstants, the ContentAPI '
    'dialogue functions, PlayerCamera, AntiMacro, the session and the zone monitor. Content activities, cutscene plugins and '
    'cutscenes are scripted oracle content (adapters.py TestActivity, TestCutscenePlugin, TestCutscene) calling only inherited '
    'original members. Every case runs in a class loader of its own; the k-th recorded call of a marked operation fails in the '
    'injected failure cases.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def markers():
    read = lambda name: c.read(SERVER / name)
    inserted = regions.markers()
    c.SOURCES.update(regions.c.SOURCES)
    api = read('core/api/ContentAPI.kt')
    location = read('core/game/world/map/Location.kt')
    inserted.update({
        'ORIGINAL_GET_ATTRIBUTE': c.method(api, 'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T {'),
        'ORIGINAL_SET_ATTRIBUTE': c.method(api, 'fun <T> setAttribute(entity: Entity, attribute: String, value: T) {'),
        'ORIGINAL_REMOVE_ATTRIBUTE': c.method(api, 'fun removeAttribute(entity: Entity, attribute: String) {'),
        'ORIGINAL_OPEN_INTERFACE': c.method(api, 'fun openInterface(player: Player, id: Int) {'),
        'ORIGINAL_CLOSE_INTERFACE': c.method(api, 'fun closeInterface(player: Player) {'),
        'ORIGINAL_GET_CUTSCENE': c.method(api, 'fun Player.getCutscene(): Cutscene? {'),
        'ORIGINAL_GET_CUTSCENE_STAGE': c.method(api, 'fun Player.getCutsceneStage(): Int {'),
        'ORIGINAL_LOCATION_TRANSFORM_DIRECTION': line(location, '@JvmOverloads fun transform(dir: Direction, steps: Int = 1)'),
    })
    return inserted


def build():
    lock = regions.verify_pins()
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libs = [stdlib]
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))['sha256']
        jar = RUN / name
        # Pinned by hash; this generator never downloads. Place the artifact here from another oracle's run directory.
        if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != pin:
            raise RuntimeError(f'{jar} is missing or differs from its pin')
        libs.append(jar)
    kotlin, java = [], []
    for name in ORIGINAL:
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
    c.read(ROOT / 'oracle/dynamic-regions/adapters.py')
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    c.read(BOOT)
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
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-d', boot, BOOT])
    return lock, os.pathsep.join(map(str, [boot, *libs])), classes


def execute(classpath, classes, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx2g', f'-Doracle.cache={CACHE}', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US',
                             '-cp', classpath, 'oracle.boot.Boot', str(classes)], input=payload, cwd=ROOT, capture_output=True, timeout=3600)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    (RUN / 'raw.jsonl').write_bytes(result.stdout)
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


def generate(check):
    cache_bytes = CACHE.read_bytes()
    pinned = json.loads((ROOT / 'fixtures/dynamic-regions/provenance.json').read_text(encoding='utf-8'))['cacheSha256']
    if hashlib.sha256(cache_bytes).hexdigest() != pinned:
        raise RuntimeError('fixtures/dynamic-regions/cache.json differs from its provenance')
    lock, classpath, classes = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, classes, cases)
    # Second pass: in each marked operation every recorded call fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at in range(1, sum(1 for e in row['expected'][sweep]['events'] if e[0] != 'stackTrace') + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, name=f"{case['name']} / failing call {at}", ops=ops))
    rows += execute(classpath, classes, failing)
    cases += failing
    (RUN / 'last.jsonl').write_text(''.join(json.dumps(row, sort_keys=True) + '\n' for row in rows), encoding='utf-8')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    c.read(ROOT / 'oracle/dynamic-regions/generate.py')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for op, step in zip(row['ops'], row['expected']):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            if 'error' in step['result']:
                errors[step['result']['error']] = errors.get(step['result']['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), events=dict(sorted(events.items())),
                    errors=dict(sorted(errors.items())), groups=corpus.groups(cases), failureCases=len(failing))
    for needed in corpus.NEEDED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    for needed in corpus.NEEDED_ERRORS:
        if needed not in errors:
            raise RuntimeError(f'the corpus never fails with {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        cacheSha256=hashlib.sha256(cache_bytes).hexdigest(), coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20',
        locale='en-US', original=ORIGINAL, scope=SCOPE)
    summary = f'{len(rows)} original activity and cutscene cases ({coverage["actions"]} actions, {len(failing)} failure cases)'
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('activity fixtures/provenance differ; inspect before regeneration')
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
    regions.c.JAVA_HOME = c.JAVA_HOME
    generate(args.check)
