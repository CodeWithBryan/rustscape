"""Run the unchanged DynamicRegion, Region, RegionChunk and RegionManager over real cache regions on JDK 11.

Compiled unchanged: DynamicRegion, ChunkFlags, LandscapeParser, MapscapeParser, Region, RegionChunk,
RegionManager.kt, Direction, ClipMaskSupplier, MapDistance, ZoneBorders, Pulse, Scenery, Constructed,
SceneryBuilder, SceneryDefinition, Definition, ByteBufferUtils, ChunkDirtyListener, UpdateFlag,
ObjectUpdateFlag, IoBuffer, PacketHeader, OutgoingPacket, Context, GameWriteEvent, IoWriteEvent,
ConstructScenery, ClearScenery, UpdateAreaPosition, their contexts, and the scene packets
BuildDynamicScene, UpdateSceneGraph, MinimapState, ClearMinimapFlag with DynamicSceneContext,
SceneGraphContext, MinimapStateContext and PlayerContext. Original text inserted at generation
time: Player.updateSceneGraph, XteaParser's key map and lookup, MapChunkRenderer's build-area
constants, StringUtils.isPlusN and Location's members. Everything else is an adapter (adapters.py).

The cache input (map and landscape bytes, scenery definitions and XTEA keys of every region the
corpus reaches) is exported from the pinned cache by `cargo xtask dynamic-regions-oracle`; this
script never reads the cache itself. --check never updates a fixture.
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
RUN = ROOT / 'runs/dynamic-regions-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/dynamic-regions'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('dynamic_regions_adapters', HERE / 'adapters.py')
corpus = load('dynamic_regions_cases', HERE / 'cases.py')

ORIGINAL = [
    'core/game/world/map/build/DynamicRegion.java',
    'core/game/world/map/build/ChunkFlags.java',
    'core/game/world/map/build/LandscapeParser.java',
    'core/game/world/map/build/MapscapeParser.java',
    'core/game/world/map/Region.java',
    'core/game/world/map/RegionChunk.java',
    'core/game/world/map/RegionManager.kt',
    'core/game/world/map/Direction.java',
    'core/game/world/map/Point.java',
    'core/game/world/map/path/ClipMaskSupplier.java',
    'core/game/world/map/MapDistance.java',
    'core/game/world/map/zone/ZoneBorders.java',
    'core/game/system/task/Pulse.java',
    'core/game/node/scenery/Scenery.java',
    'core/game/node/scenery/Constructed.java',
    'core/game/node/scenery/SceneryBuilder.java',
    'core/cache/def/impl/SceneryDefinition.java',
    'core/cache/def/Definition.java',
    'core/cache/misc/buffer/ByteBufferUtils.kt',
    'core/game/world/update/ChunkDirtyListener.kt',
    'core/game/world/update/flag/UpdateFlag.java',
    'core/game/world/update/flag/chunk/ObjectUpdateFlag.java',
    'core/net/packet/IoBuffer.java',
    'core/net/packet/PacketHeader.java',
    'core/net/packet/OutgoingPacket.java',
    'core/net/packet/Context.java',
    'core/net/event/GameWriteEvent.java',
    'core/net/IoWriteEvent.java',
    'core/net/packet/out/ConstructScenery.java',
    'core/net/packet/out/ClearScenery.java',
    'core/net/packet/out/UpdateAreaPosition.java',
    'core/net/packet/context/BuildSceneryContext.java',
    'core/net/packet/context/AreaPositionContext.java',
    'core/net/packet/out/BuildDynamicScene.java',
    'core/net/packet/out/UpdateSceneGraph.java',
    'core/net/packet/out/MinimapState.java',
    'core/net/packet/out/ClearMinimapFlag.java',
    'core/net/packet/context/DynamicSceneContext.java',
    'core/net/packet/context/SceneGraphContext.java',
    'core/net/packet/context/MinimapStateContext.java',
    'core/net/packet/context/PlayerContext.java',
]
SCOPE = (
    'Unchanged DynamicRegion, ChunkFlags, LandscapeParser, MapscapeParser, Region, RegionChunk, RegionManager, Direction, ZoneBorders, '
    'Pulse, Scenery, Constructed, SceneryBuilder, SceneryDefinition, the object update flag and writers, and the BuildDynamicScene, '
    'UpdateSceneGraph, MinimapState and ClearMinimapFlag packets with their contexts on JDK 11; Player.updateSceneGraph, XteaParser\'s '
    'lookup, the build-area constants, isPlusN and Location\'s members inserted verbatim. Real pinned cache map and landscape bytes, '
    'scenery definitions and XTEA keys of every region reached (exported by the native cache reader). Recording adapters: Entity, '
    'Player, NPC, Familiar, ZoneMonitor, the walking queue, MapZone/RegionZone/MultiwayCombatZone/MusicZone as plain records, '
    'GroundItem, GroundItemManager, Repository, GameWorld\'s pulser (a fixed-order queue), ContentAPI\'s log and queueScript, '
    'PacketRepository (reflection, no write queue), IoSession, Cache. Familiar recall scripts never run; ground items are only '
    'stored, never drawn. Not exercised: NPC.init beyond its RegionManager.move, real familiars, the region specification '
    'contracts, live sessions and the PacketWriteQueue flush order.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def markers():
    read = lambda name: c.read(SERVER / name)
    location = read('core/game/world/map/Location.kt')
    xtea = read('core/game/system/config/XteaParser.kt')
    renderer = read('core/game/world/update/MapChunkRenderer.kt')
    members = [
        line(location, 'override fun toString()'),
        line(location, 'override fun hashCode()'),
        c.method(location, 'override fun equals(other: Any?): Boolean'),
        line(location, 'val region: Region'),
        line(location, 'val regionId: Int'),
        line(location, 'val regionX: Int'),
        line(location, 'val regionY: Int'),
        line(location, 'val localX: Int'),
        line(location, 'val localY: Int'),
        line(location, 'val chunk: RegionChunk'),
        line(location, 'val chunkBase: Location'),
        line(location, 'val absChunkX: Int'),
        line(location, 'val absChunkY: Int'),
        line(location, 'val chunkX: Int'),
        line(location, 'val chunkY: Int'),
        line(location, 'val chunkOffsetX: Int'),
        line(location, 'val chunkOffsetY: Int'),
        line(location, 'val sceneX: Int'),
        line(location, 'val sceneY: Int'),
        line(location, 'fun getSceneX(loc: Location)'),
        line(location, 'fun getSceneY(loc: Location)'),
        line(location, 'fun transform(l: Location)'),
        line(location, 'fun transform(diffX: Int, diffY: Int, z: Int)'),
        c.method(location, '@JvmOverloads fun withinDistance(other: Location, dist: Int'),
    ]
    return {
        'ORIGINAL_IS_PLUS_N': c.method(read('core/tools/StringUtils.java'), 'public static boolean isPlusN('),
        'ORIGINAL_UPDATE_SCENE_GRAPH': c.method(read('core/game/node/entity/player/Player.java'), 'public void updateSceneGraph(boolean login)'),
        'ORIGINAL_REGION_XTEA': line(xtea, 'val REGION_XTEA'),
        'ORIGINAL_DEFAULT_KEYS': line(xtea, 'private val DEFAULT_REGION_KEYS'),
        'ORIGINAL_GET_REGION_XTEA': c.method(xtea, 'fun getRegionXTEA(regionId: Int)'),
        'ORIGINAL_BUILD_AREA_DEPTH': line(renderer, 'const val BUILD_AREA_DEPTH'),
        'ORIGINAL_BUILD_AREA_SIZE': line(renderer, 'const val BUILD_AREA_SIZE'),
        'ORIGINAL_GET_DISTANCE': '@JvmStatic ' + c.method(location, 'fun getDistance(first: Location, second: Location)'),
        'ORIGINAL_LOCATION_MEMBERS': '\n    '.join(members),
        'ORIGINAL_LOCATION_HASH': c.method(location, 'value class LocationHash('),
    }


def verify_pins():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    return lock


def compile_oracle():
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
    return classpath


def generate(check, cache_input, cases_only, discover=None, skip_compile=False):
    cases = corpus.requests()
    if cases_only:
        cases_only.parent.mkdir(parents=True, exist_ok=True)
        cases_only.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
        print(f'{len(cases)} dynamic region cases written.')
        return
    if cache_input is None:
        raise RuntimeError('--cache-input is required: run `cargo xtask dynamic-regions-oracle`')
    lock = verify_pins()
    classpath = compile_oracle() if not skip_compile else os.pathsep.join(map(str, [RUN / 'classes', *[p for p in c.artifacts() if p.name.startswith('kotlin-stdlib-')], RUN / 'json-simple-1.1.1.jar', RUN / 'ConstLib-1.4.28.jar']))
    cache_bytes = cache_input.read_bytes()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx2g', f'-Doracle.discover={str(bool(discover)).lower()}', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath, 'oracle.Support', cache_input]
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=1800)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if discover:
        discover.write_text(json.dumps(rows[-1]) + '\n', encoding='utf-8')
        print(f'{len(rows[-1])} unsupplied cache archives.')
        return
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')

    operations, errors, events = {}, {}, {}
    for row in rows:
        for action, step in zip(row['actions'], row['expected']['steps']):
            operations[action['op']] = operations.get(action['op'], 0) + 1
            if 'error' in step['result']:
                errors[step['result']['error']] = errors.get(step['result']['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), events=dict(sorted(events.items())),
                    errors=dict(sorted(errors.items())), groups=corpus.groups(cases))
    for needed in ['npcCreate', 'npcInit', 'npcClear', 'setInvisible', 'queueScript', 'renderable', 'log', 'zoneUpdate', 'packetError']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    for needed in ['Failure', 'ClassCast', 'Null']:
        if needed not in errors:
            raise RuntimeError(f'the corpus never fails with {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        cacheSha256=hashlib.sha256(cache_bytes).hexdigest(), coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20',
        locale='en-US', scope=SCOPE)
    summary = f'{len(rows)} original dynamic region cases ({coverage["actions"]} actions, {sum(errors.values())} failures)'
    if check:
        if ((FIXTURES / 'oracle.jsonl').read_bytes() != data or (FIXTURES / 'cache.json').read_bytes() != cache_bytes
                or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance):
            raise RuntimeError('Dynamic region fixtures/provenance differ; inspect before regeneration')
        print(f'{summary} reproduced.')
    else:
        FIXTURES.mkdir(parents=True, exist_ok=True)
        (FIXTURES / 'oracle.jsonl').write_bytes(data)
        (FIXTURES / 'cache.json').write_bytes(cache_bytes)
        (FIXTURES / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    parser.add_argument('--cache-input', type=pathlib.Path)
    parser.add_argument('--cases-only', type=pathlib.Path)
    parser.add_argument('--discover', type=pathlib.Path, help='write the cache archives the corpus reaches but the input lacks')
    parser.add_argument('--skip-compile', action='store_true', help='reuse the classes of the previous run (discovery only)')
    args = parser.parse_args()
    if args.skip_compile and not args.discover:
        raise RuntimeError('--skip-compile is only for discovery')
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check, args.cache_input, args.cases_only, args.discover, args.skip_compile)
