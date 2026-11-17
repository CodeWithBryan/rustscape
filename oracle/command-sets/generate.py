"""The command-sets oracle: the Misc, Spawn and Stats command sets, the ::update/::cancelupdate/::shutdown paths of
SystemCommandSet and the system manager (SystemManager, SystemUpdate, SystemTermination, SystemShutdownHook,
SystemConfig) run on JDK 11.

Compiled unchanged: Command, CommandSystem, CommandPlugin, the legacy CommandSet enum, Privilege, sets/CommandSet,
MiscCommandSet, SpawnCommandSet, StatsCommandSet, StatAttributeKeys, SystemCommandSet, SystemManager, SystemUpdate,
SystemTermination, SystemShutdownHook, SystemConfig, SystemState, Pulse, GameAttributes, Rights, UserAccountInfo,
EquipmentSlot, InputType, Globals, StringUtils, CP1252, Plugin and Initializable. Original text inserted at generation
time: Player.getRights and toggleDebug, PlayerDetails.getRights/getCredits/setCredits, the GlobalData accessors, the
Skills constants, WeaponInterface.STYLE_CONTROLLED, the BookInterface constants and its PageSet/Page/BookLine classes,
GE_NEWS_MUTE_ATTRIBUTE, Familiar.INFINITE_SPECIAL_MOVE_ATTRIBUTE, the BossKillCounter, Direction and IronmanMode
constant names, Scenery's two short constructors and toString, the Location members the sets read, Repository's
one-argument sendNews and the Log enum. Everything else is a recording adapter from adapters.py answering from the
case (oracle/commands' adapters copied where the same stand-in serves; that oracle is not edited). The compiled classes
pass through oracle/social's tool/Rewrite.java (wall clock reads become recorded case inputs) and oracle/social's
Launcher.java runs each case in its own class loader, headless (java.awt.headless=true, so the clipboard throws
HeadlessException as on a server) under a security manager that turns System.exit into a recorded outcome. Every
recorded call of the marked operations is then made to fail once in turn. Java/Kotlin are test oracles only; --check
never rewrites a fixture and nothing here downloads.
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
RUN = ROOT / 'runs/command-sets-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/command-sets'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('command_sets_adapters', HERE / 'adapters.py')
corpus = load('command_sets_cases', HERE / 'cases.py')

ORIGINAL = [
    'core/game/system/command/Command.kt',
    'core/game/system/command/CommandSystem.kt',
    'core/game/system/command/CommandPlugin.kt',
    'core/game/system/command/CommandSet.java',
    'core/game/system/command/Privilege.kt',
    'core/game/system/command/sets/CommandSet.kt',
    'core/game/system/command/sets/MiscCommandSet.kt',
    'core/game/system/command/sets/SpawnCommandSet.kt',
    'core/game/system/command/sets/StatsCommandSet.kt',
    'core/game/system/command/sets/StatAttributeKeys.kt',
    'core/game/system/command/sets/SystemCommandSet.kt',
    'core/game/system/SystemManager.java',
    'core/game/system/SystemUpdate.java',
    'core/game/system/SystemTermination.java',
    'core/game/system/SystemShutdownHook.java',
    'core/game/system/SystemConfig.java',
    'core/game/system/SystemState.java',
    'core/game/system/task/Pulse.java',
    'core/game/node/entity/impl/GameAttributes.java',
    'core/game/node/entity/player/info/Rights.java',
    'core/auth/UserAccountInfo.kt',
    'core/api/EquipmentSlot.kt',
    'core/api/InputType.kt',
    'core/tools/StringUtils.java',
    'core/net/packet/IoBuffer.java',
    'core/net/packet/PacketHeader.java',
    'core/net/packet/IncomingPacket.java',
    'core/cache/misc/buffer/ByteBufferUtils.kt',
    'core/tools/CP1252.java',
    'core/tools/Globals.kt',
    'core/plugin/Plugin.java',
    'core/plugin/Initializable.java',
    'core/net/packet/out/SystemUpdatePacket.java',
    'core/net/packet/out/CameraViewPacket.java',
    'core/net/packet/out/CSConfigPacket.java',
    'core/net/packet/out/UpdateAreaPosition.java',
    'core/net/packet/out/AnimateObjectPacket.java',
    'core/net/packet/context/SystemUpdateContext.java',
    'core/net/packet/context/CameraContext.java',
    'core/net/packet/context/CSConfigContext.java',
    'core/net/packet/context/AreaPositionContext.java',
    'core/net/packet/context/AnimateObjectContext.java',
    'core/net/packet/Context.java',
    'core/net/packet/OutgoingPacket.java',
]
SCOPE = (
    'On JDK 11 with Kotlin 1.8.20, each case in a fresh class loader, headless, the unchanged Command, CommandSystem, '
    'CommandPlugin, CommandSet (legacy enum), Privilege, sets.CommandSet, MiscCommandSet, SpawnCommandSet, '
    'StatsCommandSet, StatAttributeKeys, SystemCommandSet, SystemManager, SystemUpdate, SystemTermination, '
    'SystemShutdownHook, SystemConfig, SystemState, Pulse, GameAttributes, Rights, UserAccountInfo, EquipmentSlot, '
    'InputType, Globals, StringUtils, CP1252, Plugin and Initializable; original text inserted: Player.getRights and '
    'toggleDebug, PlayerDetails.getRights/getCredits/setCredits, the GlobalData accessors, the Skills constants, '
    'STYLE_CONTROLLED, the book constants and PageSet/Page/BookLine, GE_NEWS_MUTE_ATTRIBUTE, '
    'INFINITE_SPECIAL_MOVE_ATTRIBUTE, the BossKillCounter/Direction/IronmanMode constant names, Scenery\'s short '
    'constructors and toString, the Location members, Repository.sendNews(string) and the Log enum. Recording adapters: '
    'the repository, interfaces, dialogues, locks, packets, varps, containers and items, definitions, regions and chunks, '
    'scenery, NPCs, combat calculations, the GE offer queries, bots, books, rules, diaries, farming/brewing timers, '
    'trawler loot, the cutscene, quest/slayer/clue/stat counters, private messages, news, the tick worker, the pulse '
    'runner, networking, bots, shutdown listeners, world stores, PlayerMonitor and the disconnection queue. The sets '
    'register in the scan order; content commands are case-registered stand-ins. Wall clock reads are recorded case '
    'inputs; System.exit is a recorded outcome; println and printStackTrace are recorded host outputs; the working '
    'directory is written <work>. Not driven: the update thread SystemUpdate.schedule starts when the tick worker is not '
    'running, and the clipboard on a desktop (the corpus runs headless).'
)


def balanced(text, start, opening='('):
    """The text from `start` through the bracket that closes the first one after it."""
    closing = {'(': ')', '{': '}'}[opening]
    first = text.index(opening, start)
    depth = 0
    for i in range(first, len(text)):
        if text[i] == opening:
            depth += 1
        elif text[i] == closing:
            depth -= 1
            if depth == 0:
                return text[start:i + 1]
    raise RuntimeError('unbalanced brackets')


def line(text, pattern):
    match = re.search(pattern, text, re.M)
    if match is None:
        raise RuntimeError(f'a marked line moved: {pattern}')
    return match.group(0).strip()


def names(text, header):
    start = text.index(header)
    body = text[text.index('{', start) + 1:]
    body = re.sub(r'/\*.*?\*/', '', body, flags=re.S)
    body = re.sub(r'//[^\n]*', '', body)
    body = body[:body.index(';')]
    found, depth, top = [], 0, []
    for ch in body:
        if ch == '(':
            if depth == 0:
                name = re.search(r'([A-Z][A-Z0-9_]*)\s*$', ''.join(top))
                found.append(name.group(1))
                top = []
            depth += 1
        elif ch == ')':
            depth -= 1
        elif depth == 0:
            top.append(ch)
    if not found:
        raise RuntimeError(f'no constants under {header}')
    return ', '.join(found) + ';'


def markers():
    read = lambda name: c.read(SERVER / name)
    player = read('core/game/node/entity/player/Player.java')
    details = read('core/game/node/entity/player/info/PlayerDetails.java')
    global_data = read('core/game/node/entity/player/link/GlobalData.java')
    skills = read('core/game/node/entity/skill/Skills.java')
    weapon = read('core/game/node/entity/combat/equipment/WeaponInterface.java')
    book = read('content/global/handlers/iface/BookInterface.kt')
    repository = read('core/game/world/repository/Repository.kt')
    familiar = read('content/global/skill/summoning/familiar/Familiar.java')
    bosses = read('content/data/BossKillCounter.kt')
    direction = read('core/game/world/map/Direction.java')
    ironman = read('core/game/node/entity/player/link/IronmanMode.java')
    scenery = read('core/game/node/scenery/Scenery.java')
    interfaces = read('core/game/node/entity/player/link/InterfaceManager.java')
    location = read('core/game/world/map/Location.kt')
    logger = read('core/tools/SystemLogger.kt')
    location_lines = [line(location, p) for p in [
        r'^\s*override fun toString\(\): String = .*$', r'^\s*val regionId: Int get\(\) = .*$', r'^\s*val regionX: Int get\(\) = .*$',
        r'^\s*val regionY: Int get\(\) = .*$', r'^\s*val localX: Int get\(\) = .*$', r'^\s*val localY: Int get\(\) = .*$',
        r'^\s*val chunkBase: Location get\(\) = .*$', r'^\s*val chunkOffsetX: Int get\(\) = .*$', r'^\s*val chunkOffsetY: Int get\(\) = .*$',
        r'^\s*fun getSceneX\(loc: Location\): Int = .*$', r'^\s*fun getSceneY\(loc: Location\): Int = .*$',
        r'^\s*fun transform\(diffX: Int, diffY: Int, z: Int\): Location = .*$']]
    return {
        'ORIGINAL_PLAYER_GET_RIGHTS': c.method(player, 'public Rights getRights() {'),
        'ORIGINAL_TOGGLE_DEBUG': c.method(player, 'public void toggleDebug() {'),
        'ORIGINAL_DETAILS_GET_RIGHTS': c.method(details, 'public Rights getRights() {'),
        'ORIGINAL_DETAILS_CREDITS': c.method(details, 'public int getCredits() {') + '\n\n    ' + c.method(details, 'public void setCredits(int amount) {'),
        'ORIGINAL_GLOBAL_DATA': '\n\n    '.join(c.method(global_data, s) for s in [
            'public int getTestStage() {', 'public void setTestStage(int stage) {', 'public boolean[] getReadPlaques() {',
            'public boolean hasReadPlaques() {', 'public int[] getBossCounters() {', 'public int getBarrowsLoots() {']),
        'ORIGINAL_SKILL_CONSTANTS': line(skills, r'^\s*public static final int ATTACK = 0,[^;]*;'),
        'ORIGINAL_STYLE_CONTROLLED': line(weapon, r'^\s*public static final int STYLE_CONTROLLED = \d+;'),
        'ORIGINAL_BOOK_CONSTANTS': line(book, r'^\s*const val FANCY_BOOK_26 = \d+') + '\n        ' + line(book, r'^\s*val FANCY_BOOK_26_LINE_IDS = arrayOf\([^)]*\)'),
        'ORIGINAL_BOOK_CLASSES': book[book.index('/** Constructs a new PageSet object. */'):].strip(),
        'ORIGINAL_GE_NEWS_MUTE': line(repository, r'^\s*const val GE_NEWS_MUTE_ATTRIBUTE = "[^"]*"'),
        'ORIGINAL_SEND_NEWS_1': '@JvmStatic\n    ' + c.method(repository, 'fun sendNews(string: String){'),
        'ORIGINAL_INFINITE_SPECIAL_MOVE': line(familiar, r'^\s*public static final String INFINITE_SPECIAL_MOVE_ATTRIBUTE = "[^"]*";'),
        'ORIGINAL_BOSS_NAMES': names(bosses, 'enum class BossKillCounter'),
        'ORIGINAL_DIRECTION_NAMES': names(direction, 'public enum Direction'),
        'ORIGINAL_IRONMAN_NAMES': names(ironman, 'public enum IronmanMode'),
        'ORIGINAL_SCENERY_CONSTRUCTORS': c.method(scenery, 'public Scenery(int id, Location location) {') + '\n\n    '
        + c.method(scenery, 'public Scenery(int id, Location location, int rotation) {'),
        'ORIGINAL_SCENERY_TO_STRING': '@Override\n    ' + c.method(scenery, 'public String toString() {'),
        'ORIGINAL_LOCATION_MEMBERS': '\n    '.join(location_lines),
        'ORIGINAL_LOG_ENUM': c.method(logger, 'enum class Log {'),
        'ORIGINAL_GET_PACKET_COUNT': c.method(interfaces, 'public int getPacketCount(int increment) {'),
    }


def pinned(name, sha256):
    target = RUN / name
    candidates = [target, ROOT / 'runs/world-trace-oracle/jars' / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
    jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == sha256), None)
    if jar is None:
        raise RuntimeError(f'{name} matching its pin was not found under runs/; this generator never downloads')
    if jar != target:
        shutil.copyfile(jar, target)
    return target


def build():
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    annotations = next(path for path in jars if path.name.startswith('annotations-'))
    compiler = next(path for path in jars if path.name.startswith('kotlin-compiler-embeddable-'))
    json_jar = pinned('json-simple-1.1.1.jar', json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))['sha256'])
    constlib = pinned('ConstLib-1.4.28.jar', json.loads(c.read(ROOT / 'oracle/bank/constants.lock.json'))['sha256'])
    libraries = [stdlib, annotations, json_jar, constlib]
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
    for helper in ['adapters.py', 'cases.py', 'Support.java', 'Packets.java']:
        c.read(HERE / helper)
    launcher_source = ROOT / 'oracle/social/Launcher.java'
    rewrite_source = ROOT / 'oracle/social/tool/Rewrite.java'
    c.read(launcher_source)
    c.read(rewrite_source)
    java += [HERE / 'Support.java', HERE / 'Packets.java']
    compiled, classes, launcher, tool = RUN / 'compiled', RUN / 'classes', RUN / 'launcher', RUN / 'tool'
    for path in [compiled, classes, launcher, tool]:
        if path.exists():
            shutil.rmtree(path)
        path.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libraries)), '-d', compiled, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [compiled, *libraries]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', compiled, *java])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-cp', compiler, '-d', tool, rewrite_source])
    census_path = RUN / 'census.json'
    c.run(['java', '-cp', os.pathsep.join(map(str, [tool, compiler])), 'Rewrite', compiled, classes, census_path])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-d', launcher, launcher_source])
    return libraries, launcher, classes, json.loads(census_path.read_text(encoding='utf-8'))


def execute(libraries, launcher, classes, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    work = RUN / 'work'
    if work.exists():
        shutil.rmtree(work)
    work.mkdir()
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode('utf-8')
    command = [executable, '-Xmx1g', '-Djava.awt.headless=true', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US',
               '-Duser.timezone=UTC', '-cp', launcher, 'oracle.Launcher', classes, *libraries]
    result = subprocess.run(list(map(str, command)), cwd=work, input=payload, capture_output=True, timeout=3600)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:] + result.stdout.decode(errors='replace')[-500:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases: {result.stderr.decode(errors="replace")[-3000:]}')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


EXPECTED_ERRORS = (None, 'IllegalState', 'Null', 'Index', 'NumberFormat', 'Cast', 'Binding', 'Headless', 'Exit', 'NoSuchElement')


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    libraries, launcher, classes, census = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(libraries, launcher, classes, cases)
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        sweep, limit = sweep if isinstance(sweep, tuple) else (sweep, None)
        calls = [e for e in row['expected'][sweep]['events'] if e[0] not in corpus.NOTES]
        positions = range(1, len(calls) + 1)
        if limit is not None and len(calls) > limit + 5:
            # Long uniform loops (a scroll's 311 strings): the first `limit` calls and the last five.
            positions = list(range(1, limit + 1)) + list(range(len(calls) - 4, len(calls) + 1))
        for at in positions:
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    rows += execute(libraries, launcher, classes, failing)
    cases += failing
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['ops'], row['expected']):
            error = step['error'][0] if step['error'] else None
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error, (number, row['name'], op, step['error']))
            elif error == 'Binding' and 'failAt' not in op:
                mistakes.setdefault(error, (number, row['name'], op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for op, step in zip(row['ops'], row['expected']):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            if step['error']:
                errors[step['error'][0]] = errors.get(step['error'][0], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), groups=corpus.groups(cases),
                    commands=corpus.commands(cases), failureCases=len(failing))
    for needed in corpus.NEEDED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    missing = sorted(set(corpus.PORTED) - set(coverage['commands']))
    if missing:
        raise RuntimeError(f'commands never driven: {missing}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20',
        locale='en-US', timezone='UTC', scope=SCOPE, unchanged=ORIGINAL, rewrite=census,
        caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(), coverage=coverage)
    summary = (f'{len(rows)} original command-set/system-manager cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases, {len(coverage["commands"])} commands)')
    if check:
        same = (FIXTURES / 'oracle.jsonl').read_bytes() == data
        if not same or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('command-sets fixtures/provenance differ; inspect before regeneration')
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
    parser.add_argument('--compile-only', action='store_true', help='build the classes and stop (for adapter work)')
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    if args.compile_only:
        build()
        print('compiled')
    else:
        generate(args.check)
