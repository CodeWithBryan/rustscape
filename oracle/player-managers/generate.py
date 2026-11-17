"""Run the unchanged player managers on JDK 11 and record what they do.

Compiled unchanged: SkullManager, HintIconManager, WarningMessages,
AssistSession, AchievementDiaryManager, AchievementDiary, DiaryType,
DiaryLevel, Bar, the HintIcon writer with HintIconContext, IoBuffer,
PacketHeader, Context, OutgoingPacket, CloseEvent, ContainerListener,
RequestModule, MapDistance and the timer framework the Skulled timer runs on
(RSTimer, PersistTimer, TimerFlag, TimerManager, Skulled). Everything else is
a recording adapter under oracle/player-managers/adapters, with the original
text of the members named in `markers` inserted at generation time. --check
never updates a fixture, and nothing here downloads: the pinned jars must
already be under runs/.
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
RUN = ROOT / 'runs/player-managers-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/player-managers'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
corpus = load('player_managers_cases', HERE / 'cases.py')

ORIGINAL = [
    'core/game/node/entity/player/link/SkullManager.java',
    'core/game/node/entity/player/link/HintIconManager.java',
    'core/game/node/entity/player/link/WarningMessages.java',
    'core/game/node/entity/player/link/request/assist/AssistSession.java',
    'core/game/node/entity/player/link/request/RequestModule.kt',
    'core/game/node/entity/player/link/diary/AchievementDiaryManager.java',
    'core/game/node/entity/player/link/diary/AchievementDiary.java',
    'core/game/node/entity/player/link/diary/DiaryType.java',
    'core/game/diary/DiaryLevel.kt',
    'content/global/skill/smithing/smelting/Bar.java',
    'core/net/packet/out/HintIcon.java',
    'core/net/packet/context/HintIconContext.java',
    'core/net/packet/IoBuffer.java',
    'core/net/packet/PacketHeader.java',
    'core/net/packet/Context.java',
    'core/net/packet/OutgoingPacket.java',
    'core/game/component/CloseEvent.java',
    'core/game/container/ContainerListener.java',
    'core/game/world/map/MapDistance.java',
    'core/game/system/timer/RSTimer.kt',
    'core/game/system/timer/PersistTimer.kt',
    'core/game/system/timer/TimerFlag.kt',
    'core/game/system/timer/TimerManager.kt',
    'core/game/system/timer/impl/Skulled.kt',
]
SCOPE = (
    'Unchanged SkullManager (with SkullIcon and the anonymous deep-wilderness ContainerListener), HintIconManager, WarningMessages '
    '(with WarningMessage), AssistSession (with its anonymous CloseEvent), RequestModule, AchievementDiaryManager, AchievementDiary, '
    'DiaryType, DiaryLevel, Bar, the HintIcon writer with HintIconContext, IoBuffer, PacketHeader, Context, OutgoingPacket, CloseEvent, '
    'ContainerListener, MapDistance and the timer framework (RSTimer, PersistTimer, TimerFlag, TimerManager and the Skulled timer) on '
    'JDK 11 with Kotlin 1.8.20, kotlin-reflect 1.6.10 (compile only), json-simple 1.1.1 and the published ConstLib 1.4.28 (Items). '
    'Original text inserted at generation time: Player.debug and hasItem, Item.getAlchemyValue(), the GlobalData assist accessors, '
    'EquipmentContainer\'s slot constants, WildernessZone.WILDERNESS_PROT_ATTR, the Skills ids, Location.withinDistance and getDistance, '
    'ContentAPI registerTimer, spawnTimer and removeTimer. Recording adapters: Entity, Player and NPC (attributes and extensions as '
    'maps, appearance, locks, ironman restriction, username, artificial flag, messages), the three containers (host model: slots of id '
    'and amount; containsItem sums, remove takes from the first slots, add fills the first empty slot; listener lists record add and '
    'remove), Item names, ItemDefinition.getAlchemyValue, NPCDefinition names, Component (with Component.close running its event), the '
    'InterfaceManager (one opened field; close runs the opened component\'s event), PacketDispatch sendMessage/sendString/'
    'sendInterfaceConfig/sendRunScript, the session (zero ISAAC, frames recorded as bytes), setVarp, playAudio, getWorldTicks (0), log, '
    'TimerRegistry.getTimerInstance (Skulled under "skulled"), Skills refresh/getLevel/addExperience, Pulse construction, the world '
    'pulser, Animation, Graphics and ServerConstants.ENHANCED_DEEP_WILDERNESS. AssistSession\'s System is a same-package clock class '
    '(the case\'s time; the source text is unchanged). AchievementDiary.completedLevels is cleared before each case. Not exercised: '
    'TimerManager.processTimers running a due timer (no case reaches a due tick), the real InterfaceManager, packets other than '
    'HintIcon, and the content that calls these classes.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def markers():
    read = lambda name: c.read(SERVER / name)
    player = read('core/game/node/entity/player/Player.java')
    global_data = read('core/game/node/entity/player/link/GlobalData.java')
    location = read('core/game/world/map/Location.kt')
    content = read('core/api/ContentAPI.kt')
    return {
        'ORIGINAL_DEBUG': c.method(player, 'public void debug(String string)'),
        'ORIGINAL_HAS_ITEM': c.method(player, 'public boolean hasItem(Item item)'),
        'ORIGINAL_ITEM_ALCHEMY_VALUE': c.method(read('core/game/node/item/Item.java'), 'public long getAlchemyValue()'),
        'ORIGINAL_GLOBAL_ASSIST': '\n'.join(c.method(global_data, m) for m in [
            'public long getAssistTime()', 'public void setAssistTime(long assistTime)', 'public double[] getAssistExperience()',
            'public void setAssistExperience(double[] assistExperience)']),
        'ORIGINAL_EQUIPMENT_SLOTS': line(read('core/game/container/impl/EquipmentContainer.java'), 'public static final int SLOT_HAT'),
        'ORIGINAL_WILDERNESS_PROT_ATTR': line(read('core/game/world/map/zone/impl/WildernessZone.java'),
                                              'public static final String WILDERNESS_PROT_ATTR'),
        'ORIGINAL_SKILL_IDS': line(read('core/game/node/entity/skill/Skills.java'), 'public static final int ATTACK = 0'),
        'ORIGINAL_WITHIN_DISTANCE': c.method(location, '@JvmOverloads fun withinDistance(other: Location, dist: Int'),
        'ORIGINAL_GET_DISTANCE': c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location): Double'),
        'ORIGINAL_REGISTER_TIMER': c.method(content, 'fun registerTimer (entity: Entity, timer: RSTimer?)'),
        'ORIGINAL_SPAWN_TIMER': c.method(content, 'fun spawnTimer (identifier: String, vararg args: Any) : RSTimer?'),
        'ORIGINAL_REMOVE_TIMER': c.method(content, 'fun removeTimer (entity: Entity, identifier: String)'),
    }


def pinned(name, lock_path):
    pin = json.loads(c.read(ROOT / lock_path))
    candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
    for jar in candidates:
        if jar.is_file() and hashlib.sha256(jar.read_bytes()).hexdigest() == pin['sha256']:
            return jar
    raise RuntimeError(f'No {name} matching {lock_path} under runs/; this generator never downloads')


def generate(check):
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)

    lock_names = json.loads(c.read(ROOT / 'oracle/login/kotlin.lock.json'))
    for name, entry in lock_names.items():
        jar = ROOT / 'runs/kotlin-1.8.20' / name
        if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != entry['sha256']:
            raise RuntimeError(f'{jar} is missing or differs from its pin; this generator never downloads')
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    reflect = next(path for path in jars if path.name.startswith('kotlin-reflect-'))
    annotations = next(path for path in jars if path.name.startswith('annotations-'))
    libraries = [stdlib, annotations, pinned('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'),
                 pinned('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]

    kotlin, java = [], []
    for name in ORIGINAL:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    texts = {}
    for path in sorted((HERE / 'adapters').rglob('*')):
        if path.is_file():
            texts[path.relative_to(HERE / 'adapters').as_posix()] = c.read(path)
    for name, text in sorted(texts.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'cases.py')
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')

    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    compile_path = os.pathsep.join(map(str, [*libraries, reflect]))
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-classpath', compile_path, '-d', classes, *kotlin, *java])
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', os.pathsep.join(map(str, [classes, *libraries, reflect])),
           '-d', classes, *java])
    classpath = os.pathsep.join(map(str, [classes, *libraries]))

    cases = corpus.requests()
    request_path = RUN / 'input.jsonl'
    request_path.write_text(''.join(json.dumps(case) + '\n' for case in cases), encoding='utf-8')
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    command = [executable, '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath, 'oracle.Support']
    with request_path.open('rb') as stdin:
        result = subprocess.run(list(map(str, command)), cwd=ROOT, stdin=stdin, capture_output=True, timeout=1200)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace')[-6000:] + result.stdout.decode(errors='replace')[-2000:])
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
            events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(
        actions=sum(operations.values()),
        operations=dict(sorted(operations.items())),
        recordedCalls=sum(events.values()),
        events=dict(sorted(events.items())),
        errors=dict(sorted(errors.items())),
        groups=corpus.groups(cases),
        injectedFailures=sum(len(row.get('failures', {})) for row in rows),
    )
    for needed in ['skullIcon', 'updateAppearance', 'appearanceSync', 'lockTeleport', 'unlockTeleport', 'timerInstance', 'listenerAdd',
                   'listenerRemove', 'toArray', 'alchemy', 'setAttribute', 'removeAttribute', 'send', 'location', 'index', 'open', 'close',
                   'componentClose', 'restoreTabs', 'varp', 'message', 'string', 'ifaceConfig', 'runScript', 'audio', 'animate', 'graphics',
                   'face', 'lock', 'unlock', 'submit', 'clock', 'ironmanRestriction', 'addExtension', 'removeExtension', 'globalData',
                   'skillsRefresh', 'level', 'addExperience', 'sendMessage', 'sendMessages', 'npcName', 'openTab', 'isOpened', 'contains',
                   'remove', 'add', 'freeSlots', 'get', 'itemName', 'isArtificial', 'stackTrace']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    for needed in ['Failure', 'Null', 'ClassCast', 'Index']:
        if needed not in errors:
            raise RuntimeError(f'the corpus never fails with {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', scope=SCOPE)
    summary = (f'{len(rows)} original player manager cases ({coverage["actions"]} actions, '
               f'{coverage["recordedCalls"]} recorded calls, {sum(errors.values())} failures)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Player manager fixtures/provenance differ; inspect before regeneration')
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
