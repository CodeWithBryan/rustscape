"""Run the original artificial player engine on JDK 11 and record what it does.

Compiled unchanged: the seventeen files of core/game/bots (AIPlayer, ArtificialSession, AIRepository, PlayerCompatible,
PlayerScripts, Script, ScriptAPI, ScriptDescription, ScriptIdentifier, ScriptName, AIPBuilder, GeneralBotCreator,
CombatBot, CombatBotAssembler, SkillingBotAssembler, PvMBots, PvMBotsBuilder) with the original NodeList,
NodeListIterator, Location, Vector, Direction, Item, IntType, Quests and Globals. Everything else they reach is a
recording stand-in of adapters.py; the members named in `markers` are original text inserted at generation time. Every
case runs in a class loader of its own (Boot.java), so AIPlayer's static initialiser and the engine's statics start
fresh. Java and Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/bots-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/bots'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('bots_adapters', HERE / 'adapters.py')
corpus = load('bots_cases', HERE / 'cases.py')

B = 'core/game/bots/'
UNCHANGED = [
    B + 'AIPlayer.java',
    B + 'ArtificialSession.java',
    B + 'AIRepository.kt',
    B + 'PlayerCompatible.kt',
    B + 'PlayerScripts.kt',
    B + 'Script.java',
    B + 'ScriptAPI.kt',
    B + 'ScriptDescription.kt',
    B + 'ScriptIdentifier.kt',
    B + 'ScriptName.kt',
    B + 'AIPBuilder.java',
    B + 'GeneralBotCreator.kt',
    B + 'CombatBot.kt',
    B + 'CombatBotAssembler.kt',
    B + 'SkillingBotAssembler.kt',
    B + 'PvMBots.java',
    B + 'PvMBotsBuilder.kt',
    'core/game/world/repository/NodeList.java',
    'core/game/world/repository/NodeListIterator.java',
    'core/game/world/map/Location.kt',
    'core/api/utils/Vector.kt',
    'core/game/world/map/Direction.java',
    'core/game/node/item/Item.java',
    'core/game/interaction/IntType.kt',
    'core/tools/Globals.kt',
    'content/data/Quests.kt',
]
SCOPE = (
    'The seventeen files of core/game/bots compiled unchanged on JDK 11 with Kotlin 1.8.20, JSON-simple 1.1.1 and the '
    'published ConstLib 1.4.28 (the Items names used checked against their suffixes), together with the original NodeList, '
    'NodeListIterator, Location, Vector, Direction, Item, IntType, Quests and Globals. Original text inserted into stand-ins at '
    'generation time: Player clear, finishClear, sendLogoutEvents, setPlaying, isPlaying, isArtificial, getName and '
    'getUsername, StringUtils.formatDisplayName, the Skills ids, the EquipmentContainer slots, the Log constants and '
    'ItemConfigParser.GE_PRICE. Everything else is a recording stand-in answering from the case: every other member of '
    'Player, Entity, Node, NPC and Scenery, the player\'s managers and containers, item definitions, pulses and pulse '
    'managers, pathfinding, RegionManager, interaction plugins and listeners, the Grand Exchange, consumables, '
    'RandomFunction, the session base class, ContentAPI\'s log/getWorldTicks/itemDefinition, the content scripts (CaseScript, '
    'Idler, Adventurer) and the pest control bots. Stand-ins declared in core.game.bots (a class Random and two random() '
    'extensions) are what the unchanged sources resolve `Random()` and `.random()` to, so those draws are recorded calls. '
    'kotlin.random.Random.nextInt(from, until) cannot be replaced or seeded: its two draws in randomizeLocationInRanges are read '
    'off the result, and the corpus only gives it ranges of one value. Name files are real files read by the original Scanner. The disconnection queue keeps what Player.clear '
    'writes and is compared as state. kotlinx.coroutines is a stand-in that keeps a launched block until the case runs '
    'it. Each case runs in a class loader of its own.'
)


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if not found:
        raise RuntimeError(f'no line starts with {prefix}')
    return '\n    '.join(found)


def markers():
    read = lambda name: c.read(SERVER / name)
    player = read('core/game/node/entity/player/Player.java')
    skills = read('core/game/node/entity/skill/Skills.java')
    logger = read('core/tools/SystemLogger.kt')
    body = logger[logger.index('enum class Log {') + len('enum class Log {'):]
    body = body[:body.index('}')]
    config = read('core/game/system/config/ItemConfigParser.kt')
    price = re.search(r'const val GE_PRICE = ("[^"]*")', config).group(1)
    return {
        'PLAYER_CLEAR': c.method(player, 'public void clear()'),
        'PLAYER_FINISH_CLEAR': c.method(player, 'public void finishClear()'),
        'PLAYER_SEND_LOGOUT_EVENTS': c.method(player, 'private void sendLogoutEvents()'),
        'PLAYER_SET_PLAYING': c.method(player, 'public void setPlaying(boolean playing)'),
        'PLAYER_IS_PLAYING': c.method(player, 'public boolean isPlaying()'),
        'PLAYER_IS_ARTIFICIAL': c.method(player, 'public boolean isArtificial()'),
        'PLAYER_GET_NAME': c.method(player, 'public String getName()'),
        'PLAYER_GET_USERNAME': c.method(player, 'public String getUsername()'),
        'FORMAT_DISPLAY_NAME': c.method(read('core/tools/StringUtils.java'), 'public static String formatDisplayName(String name)'),
        'SKILL_IDS': lines(skills, 'public static final int ATTACK = 0,') + '\n    ' + lines(skills, 'public static final int NUM_SKILLS ='),
        'EQUIPMENT_SLOTS': lines(read('core/game/container/impl/EquipmentContainer.java'), 'public static final int SLOT_HAT = 0,'),
        'LOG_CONSTANTS': ', '.join(w.strip() for w in body.split(',') if w.strip()),
        'GE_PRICE': f'public static final String GE_PRICE = {price};',
    }


def libraries():
    """Pinned jars: this run directory first, then any other oracle's run directory, each checked against its pin."""
    found = []
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))['sha256']
        candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
        jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
        if jar is None:
            # Pinned by hash; this generator never downloads.
            raise RuntimeError(f'{name} matching its pin was not found in runs/bots-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def constants_match_names(classpath, texts):
    """Every ConstLib constant the compiled sources name carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    sources = ''.join(texts)
    text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.Items'], capture_output=True, check=True).stdout.decode()
    used = sorted(set(re.findall(r'(?<![\w.])Items\.([A-Z][A-Z0-9_]*_\d+)\b', sources)))
    for name in used:
        found = re.search(r'int ' + name + r' = (-?\d+);', text)
        if found is None or found.group(1) != name.rsplit('_', 1)[1]:
            raise RuntimeError(f'Items.{name} differs from its suffix')
    return used


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
    kotlin, java, texts = [], [], []
    for name in UNCHANGED:
        path = SERVER / name
        texts.append(c.read(path))
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    c.read(HERE / 'adapters.py')
    used = set()
    for name, text in sorted(adapters.ADAPTERS.items()):
        for marker, original in inserted.items():
            if f'/*{marker}*/' in text:
                used.add(marker)
                text = text.replace(f'/*{marker}*/', original)
        if re.search(r'/\*[A-Z_]+\*/', text):
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    if used != set(inserted):
        raise RuntimeError(f'Unused markers {sorted(set(inserted) - used)}')
    c.read(HERE / 'cases.py')
    for name in ('R.java', 'Support.java'):
        c.read(HERE / name)
        java.append(HERE / name)
    c.read(HERE / 'Boot.java')
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
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-d', boot, HERE / 'Boot.java'])
    checked = constants_match_names(os.pathsep.join(map(str, libs)), texts)
    return lock, os.pathsep.join(map(str, [boot, *libs])), classes, checked


def execute(classpath, classes, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    data = RUN / 'data'
    if data.exists():
        shutil.rmtree(data)
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', f'-Doracle.data={data}',
                             '-cp', classpath, 'oracle.boot.Boot', str(classes)], input=payload, cwd=ROOT, capture_output=True, timeout=1800)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Cast', 'Index', 'Number', 'Argument', 'NoSuchElement', 'Binding')


def generate(check):
    lock, classpath, classes, checked = build()
    planned = list(corpus.cases())
    names = [case['name'] for case, _ in planned]
    if len(set(names)) != len(names):
        raise RuntimeError(f'duplicate case names {sorted(n for n in set(names) if names.count(n) > 1)}')
    cases = [case for case, _ in planned]
    rows = execute(classpath, classes, cases)
    # Second pass: in each marked operation every recorded call fails once in turn.
    # A long operation is sampled: `(index, stride)` fails every stride-th call, starting with the first.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        sweep, stride = sweep if isinstance(sweep, tuple) else (sweep, 1)
        counted = [e for e in row['expected'][sweep]['events'] if not e[0].startswith('@')]
        for at in range(1, len(counted) + 1, stride):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops, name=f'{case["name"]}/fail{at}'))
    rows += execute(classpath, classes, failing)
    cases += failing
    (RUN / 'last.jsonl').write_text(''.join(json.dumps(row, sort_keys=True) + '\n' for row in rows), encoding='utf-8')
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['ops'], row['expected']):
            error = step['error']
            where = f'{row.get("name")} op {row["ops"].index(op)} {op["op"]}'
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error[:300], where)
            elif error == 'Binding' and 'failAt' not in op:
                mistakes[where] = error
            # A designed operation ends as it says: with the named failure class, or with none.
            if not any('failAt' in o for o in row['ops']) and error in EXPECTED_ERRORS and error != op.get('expect'):
                mistakes[where] = f'{error} instead of {op.get("expect")}'
    if mistakes:
        raise RuntimeError('unexpected outcomes (corpus or stand-in mistakes):\n' + '\n'.join(f'  {k}: {v}' for k, v in list(mistakes.items())[:40]))
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for op, step in zip(row['ops'], row['expected']):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            if step['error']:
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(actions=sum(operations.values()), operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), groups=corpus.groups(cases),
                    failureCases=len(failing))
    missing = [e for e in corpus.NEEDED_EVENTS if e not in events] + [o for o in corpus.NEEDED_OPERATIONS if o not in operations]
    if missing:
        raise RuntimeError(f'the corpus never reaches {missing}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED,
        constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), scope=SCOPE)
    summary = (f'{len(rows)} original bot engine cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('bot engine fixtures/provenance differ; inspect before regeneration')
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
    parser.add_argument('--compile-only', action='store_true')
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    if args.compile_only:
        build()
        print('compiled')
    else:
        generate(args.check)
