"""Run the original NPC subclass framework and global NPC handlers on JDK 11 and record what they do.

Compiled unchanged: AbstractNPC and IdleAbstractNPC (core/game/node/entity/npc), the generic NPC classes ShopNPC,
BankerNPC (with its listeners, destination override and companion), CowNPC, DuckNPC, RatNPC, GuardNPC, MonkNPC,
HighwayManNPC, IceGiantNPC and DarkWizardNPC, the listeners NPCTalkListener, NPCDisturbPlugin and HairCutPlugin, the
gnome cooking job table (GnomeCookingJob, GnomeCookingConstants), with the unchanged OptionHandler, Plugin, IntType,
core.api.Container and Direction. The members named in `markers` are original text inserted into the adapters of
adapters.py at generation time (NPC.create, the Skills ids, BankBoothListener.BANK_BOOTHS, the BarcrawlType constants'
NPC ids and BarcrawlType.forId). Everything else is a recording adapter answering from the case.
Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/npc-classes-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/npc-classes'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('npc_classes_adapters', HERE / 'adapters.py')
corpus = load('npc_classes_cases', HERE / 'cases.py')

H = 'content/global/handlers/npc/'
UNCHANGED = [
    'core/game/node/entity/npc/AbstractNPC.java',
    'core/game/node/entity/npc/IdleAbstractNPC.java',
    H + 'ShopNPC.java',
    H + 'BankerNPC.kt',
    H + 'NPCTalkListener.kt',
    H + 'CowNPC.java',
    H + 'DuckNPC.java',
    H + 'RatNPC.java',
    H + 'GuardNPC.java',
    H + 'MonkNPC.java',
    H + 'HighwayManNPC.java',
    H + 'IceGiantNPC.java',
    H + 'DarkWizardNPC.java',
    H + 'NPCDisturbPlugin.java',
    H + 'HairCutPlugin.java',
    'content/minigame/gnomecooking/GnomeCookingJob.kt',
    'content/minigame/gnomecooking/GnomeCookingConstants.kt',
    'core/game/interaction/OptionHandler.java',
    'core/plugin/Plugin.java',
    'core/game/interaction/IntType.kt',
    'core/api/Container.kt',
    'core/game/world/map/Direction.java',
]
SCOPE = (
    'The NPC classes and listeners listed in `unchanged` compiled unchanged on JDK 11 with Kotlin 1.8.20 and the '
    'published ConstLib 1.4.28 (NPCs, Items and Scenery values checked against the suffixes of the names used), with '
    'the unchanged OptionHandler, Plugin, IntType, core.api.Container and Direction. Original text inserted into '
    'adapters at generation time: NPC.create (both overloads), the Skills ids, BankBoothListener.BANK_BOOTHS, the '
    'BarcrawlType constants with their NPC ids and BarcrawlType.forId. Everything else is a recording adapter '
    'answering from the case: the NPC constructor and the NPC/Entity members the classes reach (init, tick, '
    'handleTickActions, finalizeDeath, onAttack, onImpact, getSwingHandler, walks, aggression, direction, transform, '
    'attributes, chat), Location (the constructor plane and 15-bit packing of Location.kt, its transforms and '
    'withinDistance), the InteractionListeners registry (on/setDest recorded in order), NPCDefinition handlers and '
    'option handlers, the dialogue interpreter and DialogueLabeller.open, ContentAPI, the combat pulse, swing '
    'handlers, skills, quests, diaries, ground items, spells, the zone monitor, DeathTask, random functions, the random '
    'event and holiday NPC lookups, the barcrawl manager and the gnome cooking completion dialogue. The clock the '
    'original reads itself (IdleAbstractNPC) is recorded relative to it in whole seconds.'
)


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if not found:
        raise RuntimeError(f'no line starts with {prefix}')
    return '\n    '.join(found)


def markers():
    read = lambda name: c.read(SERVER / name)
    npc = read('core/game/node/entity/npc/NPC.java')
    booth = read('content/global/handlers/scenery/BankBoothListener.kt')
    booths = re.search(r'val BANK_BOOTHS = intArrayOf\([^)]*\)', booth)
    if booths is None:
        raise RuntimeError('BankBoothListener.BANK_BOOTHS changed')
    barcrawl = read('content/region/kandarin/barcrawl/BarcrawlType.java')
    types = re.findall(r'^    ([A-Z_]+)\((\d+|new int\[\]\{[\d, ]+\}),', barcrawl, re.M)
    if len(types) != 10:
        raise RuntimeError(f'BarcrawlType constants changed: {types}')
    constants = ', '.join(f'{name}({ids.replace("new int[]{", "").replace("}", "")})' for name, ids in types)
    return {
        'ORIGINAL_NPC_CREATE': c.method(npc, 'public static NPC create(int id, Location location, Direction direction, Object... objects) {')
        + '\n    ' + c.method(npc, 'public static NPC create(int id, Location location, Object... objects) {'),
        'ORIGINAL_SKILL_IDS': lines(read('core/game/node/entity/skill/Skills.java'), 'public static final int ATTACK = 0,'),
        'ORIGINAL_BANK_BOOTHS': booths.group(0),
        'ORIGINAL_BARCRAWL_TYPES': constants,
        'ORIGINAL_BARCRAWL_FOR_ID': c.method(barcrawl, 'public static BarcrawlType forId(int id) {'),
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/npc-classes-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


CONSTANT_KINDS = ['Items', 'NPCs', 'Scenery', 'Sounds', 'Animations']


def constants_match_names(classpath, texts):
    """Every ConstLib constant the compiled sources name carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    sources = ''.join(texts)
    checked = {}
    for kind in CONSTANT_KINDS:
        text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.' + kind], capture_output=True, check=True).stdout.decode()
        used = sorted(set(re.findall(r'(?<![\w.])' + kind + r'\.([A-Z][A-Z0-9_]*_\d+)\b', sources)))
        for name in used:
            found = re.search(r'int ' + name + r' = (-?\d+);', text)
            if found is None or found.group(1) != name.rsplit('_', 1)[1]:
                raise RuntimeError(f'{kind}.{name} differs from its suffix')
        checked[kind] = used
    return checked


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    reference = ROOT / 'reference' / 'server'
    if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock['server']['commit']:
        raise RuntimeError('Reference pin differs')
    c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libs = [stdlib, *libraries()]
    kotlin, java, texts = [], [], []
    for name in UNCHANGED:
        path = SERVER / name
        texts.append(c.read(path))
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
        texts.append(text)
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
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-XDignore.symbol.file', '-cp', classpath, '-d', classes, *java])
    checked = constants_match_names(os.pathsep.join(map(str, libs)), texts)
    return lock, classpath, checked


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Support'], input=payload, cwd=ROOT, capture_output=True, timeout=900)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.decode('utf-8').splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    for row, case in zip(rows, cases):
        if {k: v for k, v in row.items() if k != 'expected'} != case:
            raise RuntimeError('the oracle transport changed a case')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Index', 'Cast', 'Binding')


def generate(check):
    lock, classpath, checked = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, cases)
    # Second pass: in each marked operation every recorded call fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at in range(1, len(row['expected'][sweep]['events']) + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    rows += execute(classpath, failing)
    cases += failing
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['ops'], row['expected']):
            error = step['error']
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error, (number, row.get('name'), op))
            elif error == 'Binding' and 'failAt' not in op and not corpus.injects(row):
                mistakes.setdefault(error, (number, row.get('name'), op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
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
    for needed in corpus.NEEDED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED,
        constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), scope=SCOPE)
    summary = (f'{len(rows)} original NPC class cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('npc class fixtures/provenance differ; inspect before regeneration')
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
