"""Run the original core/api functions on JDK 11 and record what they do.

Compiled unchanged: core/api/ContentAPI.kt and the other core/api files listed in UNCHANGED (ApiExtensions, Commands,
Container, ContentInterface, DialUtils, EquipmentSlot, Event, God, IfaceSettingsBuilder, InputType, the listener
interfaces, MapArea, regionspec, and the utils Vector, WeightedTable, CameraUtils, Exceptions and PlayerCamera), with the
original Location, Script, Clocks, Globals, TickUtils, DiaryLevel, ForceMoveCtx, Quests and StorableFamily they use.
Every other class they reach is a recording adapter (adapters_*.py) answering from the case; the members named in
`markers` are original text inserted into those adapters at generation time. Each case runs in a class loader of its
own (Boot.java). Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/content-api-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/content-api'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
corpus = load('content_api_cases', HERE / 'cases.py')
ADAPTER_MODULES = ['adapters_nodes.py', 'adapters_link.py', 'adapters_kotlin.py']

A = 'core/api/'
UNCHANGED = [
    A + 'ContentAPI.kt',
    A + 'ApiExtensions.kt',
    A + 'Commands.kt',
    A + 'Container.kt',
    A + 'ContentInterface.kt',
    A + 'DialUtils.kt',
    A + 'EquipmentSlot.kt',
    A + 'Event.kt',
    A + 'God.kt',
    A + 'IfaceSettingsBuilder.kt',
    A + 'InputType.kt',
    A + 'LoginListener.kt',
    A + 'LogoutListener.kt',
    A + 'MapArea.kt',
    A + 'PersistPlayer.kt',
    A + 'PersistWorld.kt',
    A + 'ShutdownListener.kt',
    A + 'StartupListener.kt',
    A + 'TickListener.kt',
    A + 'regionspec/RegionSpecification.kt',
    A + 'regionspec/contracts/ChunkSpecContract.kt',
    A + 'regionspec/contracts/CloneRegionContract.kt',
    A + 'regionspec/contracts/EmptyChunkContract.kt',
    A + 'regionspec/contracts/EmptyRegionContract.kt',
    A + 'regionspec/contracts/FillChunkContract.kt',
    A + 'regionspec/contracts/RegionSpecContract.kt',
    A + 'regionspec/contracts/UseExistingRegionContract.kt',
    A + 'utils/Vector.kt',
    A + 'utils/WeightedTable.kt',
    A + 'utils/CameraUtils.kt',
    A + 'utils/Exceptions.kt',
    A + 'utils/PlayerCamera.kt',
    'core/game/world/map/Location.kt',
    'core/game/interaction/Script.kt',
    'core/game/interaction/Clocks.kt',
    'core/tools/Globals.kt',
    'core/tools/TickUtils.kt',
    'core/game/diary/DiaryLevel.kt',
    'core/game/world/update/flag/context/ForceMoveCtx.kt',
    'content/data/Quests.kt',
    'content/global/skill/construction/decoration/pohstorage/StorableFamily.kt',
]
SCOPE = (
    'core/api/ContentAPI.kt and the other core/api files listed in `unchanged` compiled unchanged on JDK 11 with Kotlin '
    '1.8.20 and the published ConstLib 1.4.28, together with the original Location, Script, Clocks, Globals, TickUtils, '
    'DiaryLevel, ForceMoveCtx, Quests and StorableFamily. Original text inserted into adapters at generation time: '
    'Skills.SKILL_NAME and the skill ids, Direction.getDirection(int, int) and the QuestRequirements table. Everything '
    'else is a recording adapter answering from the case: every member of Player, Entity, NPC, Item, Scenery and the '
    'containers, managers, dispatchers, definitions, registries, timers, scripts, pulses and content objects ContentAPI '
    'reaches. Item, Scenery and Animation construction is recorded; varp maps, clocks, script fields, charges and the '
    'poison timer are case state, snapshotted after the call. Each case runs in a class loader of its own; follow-up '
    'steps run the recorded pulses, queued scripts and skill dialogue callbacks. Calls made while a class initialises '
    'are not recorded.'
)


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if not found:
        raise RuntimeError(f'no line starts with {prefix}')
    return '\n    '.join(found)


def markers():
    read = lambda name: c.read(SERVER / name)
    skills = read('core/game/node/entity/skill/Skills.java')
    requirement = read('core/game/requirement/Requirement.kt')
    return {
        'ORIGINAL_SKILL_NAME': lines(skills, 'public static final String[] SKILL_NAME = {') + '\n    ' + lines(skills, 'public static final int ATTACK = 0,'),
        'ORIGINAL_ITEM_TO_STRING': c.method(read('core/game/node/item/Item.java'), 'public String toString() {'),
        'ORIGINAL_DIRECTION_GET': c.method(read('core/game/world/map/Direction.java'), 'public static Direction getDirection(int diffX, int diffY) {'),
        'ORIGINAL_QUEST_REQUIREMENTS': c.method(requirement, 'enum class QuestRequirements(val quest: Quests, vararg val requirements: Requirement) {'),
    }


def quest_requirements():
    """The QuestRequirements table as the original declares it: (constant, quest) in declaration order."""
    text = c.method(c.read(SERVER / 'core/game/requirement/Requirement.kt'), 'enum class QuestRequirements(val quest: Quests, vararg val requirements: Requirement) {')
    return [list(m) for m in re.findall(r'^\s*([A-Z_0-9]+)\s*\(\s*Quests\.([A-Z_0-9]+)', text, re.M)]


def quest_names():
    """`Quests` constants and their names (`toString()`), in declaration order."""
    text = c.read(SERVER / 'content/data/Quests.kt')
    return [list(m) for m in re.findall(r'^\s+([A-Z_0-9]+)\("([^"]*)"\)', text, re.M)]


def libraries():
    """Pinned jars: this run directory first, then any other oracle's run directory, each checked against its pin."""
    found = []
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))['sha256']
        candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
        jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
        if jar is None:
            # Pinned by hash; this generator never downloads.
            raise RuntimeError(f'{name} matching its pin was not found in runs/content-api-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


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
    for module in ADAPTER_MODULES:
        c.read(HERE / module)
        for name, text in sorted(load(module[:-3], HERE / module).ADAPTERS.items()):
            for marker, original in inserted.items():
                text = text.replace(f'/* {marker} */', original)
            if 'ORIGINAL_' in text:
                raise RuntimeError(f'Unfilled marker in {name}')
            target = folder / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, encoding='utf-8', newline='\n')
            (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'cases.py')
    for name in ('R.java', 'Support.java'):
        c.read(HERE / name)
        java.append(HERE / name)
    c.read(HERE / 'Calls.kt')
    kotlin.append(HERE / 'Calls.kt')
    c.read(HERE / 'Boot.java')
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-XDignore.symbol.file', '-cp', classpath, '-d', classes, *java])
    boot = RUN / 'boot'
    if boot.exists():
        shutil.rmtree(boot)
    boot.mkdir()
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-d', boot, HERE / 'Boot.java'])
    return lock, os.pathsep.join(map(str, [boot, *libs])), classes


def execute(classpath, classes, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.boot.Boot', str(classes)], input=payload, cwd=ROOT, capture_output=True, timeout=1800)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Index', 'Cast', 'Binding', 'NoSuchElement', 'Argument', 'State', 'Lateinit')


def generate(check):
    lock, classpath, classes = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, classes, cases)
    # Second pass: in each marked case every recorded call of the operation fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if not sweep:
            continue
        recorded = [e for e in row['expected']['events'] if e[0] not in ('@pulse', '@script', '@printStackTrace')]
        for at in range(1, len(recorded) + 1):
            failing.append(dict(case, failAt=at, name=case['name'] + f'/fail{at}'))
    rows += execute(classpath, classes, failing)
    cases += failing
    (RUN / 'last.jsonl').write_text(''.join(json.dumps(row, sort_keys=True) + '\n' for row in rows), encoding='utf-8')
    mistakes = {}
    for row in rows:
        error = row['expected']['error']
        if error not in EXPECTED_ERRORS:
            mistakes.setdefault(error, row['name'])
        elif error == 'Binding' and 'failAt' not in row:
            mistakes.setdefault(error, row['name'])
        for step in row['expected']['steps']:
            if step[1] not in EXPECTED_ERRORS:
                mistakes.setdefault(step[1], row['name'])
        for want in row.get('want', []):
            pass
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    functions, events, errors = {}, {}, {}
    for row in rows:
        functions[row['fn']] = functions.get(row['fn'], 0) + 1
        if row['expected']['error']:
            errors[row['expected']['error']] = errors.get(row['expected']['error'], 0) + 1
        for event in row['expected']['events']:
            events[event[0]] = events.get(event[0], 0) + 1
    missing = sorted(set(corpus.FUNCTIONS) - set(functions))
    if missing:
        raise RuntimeError(f'the corpus never calls {missing}')
    coverage = dict(cases=len(rows), functions=dict(sorted(functions.items())), recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), failureCases=len(failing))
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED,
        questRequirements=quest_requirements(), quests=quest_names(), scope=SCOPE)
    summary = f'{len(rows)} original core/api cases ({len(functions)} entry points, {coverage["recordedCalls"]} recorded calls, {len(failing)} failure cases)'
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('core/api fixtures/provenance differ; inspect before regeneration')
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
