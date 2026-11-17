"""Run the original summoning familiar framework on JDK 11 and record what it does.

Compiled unchanged: FamiliarManager, Familiar, BurdenBeast, BurdenContainerListener, Forager and FamiliarSpecial
(content/global/skill/summoning/familiar), Pet, Pets and PetDetails (content/global/skill/summoning/pet) and
core/plugin/Plugin. Everything they reach outside those files is a recording stand-in of adapters.py (named after the
native `world::familiars::Host` services); the content classes of the corpus (oracle.CaseFamiliar, CaseBeast,
CaseForager) extend the unchanged abstract classes and answer from the case. The members named in `markers` are
original text inserted at generation time. Every case runs in a class loader of its own (Boot.java), so the static
FAMILIARS registry and the Pets maps start fresh. Java is a test oracle only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/familiars-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/familiars'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('familiars_adapters', HERE / 'adapters.py')
corpus = load('familiars_cases', HERE / 'cases.py')

S = 'content/global/skill/summoning/'
UNCHANGED = [
    S + 'familiar/FamiliarManager.java',
    S + 'familiar/Familiar.java',
    S + 'familiar/BurdenBeast.java',
    S + 'familiar/BurdenContainerListener.java',
    S + 'familiar/Forager.java',
    S + 'familiar/FamiliarSpecial.java',
    S + 'pet/Pet.java',
    S + 'pet/Pets.java',
    S + 'pet/PetDetails.java',
    'core/plugin/Plugin.java',
    'core/game/component/CloseEvent.java',
]
SCOPE = (
    'FamiliarManager, Familiar, BurdenBeast, BurdenContainerListener, Forager, FamiliarSpecial, Pet, Pets, PetDetails, '
    'Plugin and CloseEvent compiled unchanged on JDK 11 with JSON-simple 1.1.1 and the published ConstLib 1.4.28 (the Sounds and Items '
    'names used checked against their suffixes). Original text inserted into stand-ins at generation time: '
    'WeaponInterface.STYLE_DEFENSIVE, the Skills ids, the Log constants, ItemConfigParser.BANKABLE and Location.getDistance '
    '(transcribed to Java beside it). Everything else is a recording stand-in answering from the case: the NPC and entity '
    'members (each node keeps its location, visibility, ids and attributes), the player\'s managers (skills, packet '
    'dispatch, interfaces, dialogue, appearance, quests, skull, zones, locks, combat pulse, pulse manager), containers, '
    'item and NPC definitions, ContentAPI, RegionManager.getSpawnLocation, GameWorld (ticks and the pulser), '
    'RandomFunction, Projectile, BattleState, CombatSwingHandler, SummoningPouch, SummoningScroll, SkillcapePerks and '
    'the packet repository. The content classes of the corpus (CaseFamiliar, CaseBeast, CaseForager) extend the '
    'unchanged classes and record construct, specialMove, getIds, configureFamiliar, getText, handleFamiliarTick, '
    'adjustPlayerBattle and getRandom. Each case runs in a class loader of its own.'
)


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if not found:
        raise RuntimeError(f'no line starts with {prefix}')
    return '\n    '.join(found)


def markers():
    read = lambda name: c.read(SERVER / name)
    logger = read('core/tools/SystemLogger.kt')
    body = logger[logger.index('enum class Log {') + len('enum class Log {'):]
    body = body[:body.index('}')]
    config = read('core/game/system/config/ItemConfigParser.kt')
    bankable = re.search(r'const val BANKABLE = ("[^"]*")', config).group(1)
    location = read('core/game/world/map/Location.kt')
    return {
        'STYLE_DEFENSIVE': lines(read('core/game/node/entity/combat/equipment/WeaponInterface.java'), 'public static final int STYLE_DEFENSIVE ='),
        'SKILL_IDS': lines(read('core/game/node/entity/skill/Skills.java'), 'public static final int ATTACK = 0,'),
        'LOG_CONSTANTS': ', '.join(w.strip() for w in body.split(',') if w.strip()),
        'BANKABLE': f'public static final String BANKABLE = {bankable};',
        'GET_DISTANCE': '/* ' + c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location): Double {') + ' */',
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/familiars-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def constants_match_names(classpath, texts):
    """Every ConstLib constant the compiled sources name carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    sources = ''.join(texts)
    checked = {}
    for kind in ['Items', 'Sounds']:
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
    java, texts = [], []
    for name in UNCHANGED:
        path = SERVER / name
        texts.append(c.read(path))
        java.append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(adapters.ADAPTERS.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/*{marker}*/', original)
        if re.search(r'/\*[A-Z_]+\*/', text):
            raise RuntimeError(f'Unfilled marker in {name}')
        texts.append(text)
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        java.append(target)
    c.read(HERE / 'adapters.py')
    c.read(HERE / 'cases.py')
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    c.read(HERE / 'Boot.java')
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
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


EXPECTED_ERRORS = (None, 'Null', 'Cast', 'Number', 'Index', 'Binding')


def generate(check):
    lock, classpath, classes, checked = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, classes, cases)
    # Second pass: in each marked operation every recorded call fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at in range(1, len(row['expected'][sweep]['events']) + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    rows += execute(classpath, classes, failing)
    cases += failing
    (RUN / 'last.jsonl').write_text(''.join(json.dumps(row, sort_keys=True) + '\n' for row in rows), encoding='utf-8')
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['ops'], row['expected']):
            error = step['error']
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error, (number, row.get('name'), op))
            elif error == 'Binding' and 'failAt' not in op and not corpus.injects(row):
                mistakes.setdefault(error, (number, row.get('name'), op))
            elif error in corpus.UNEXPECTED and not op.get('expect') == error and 'failAt' not in op:
                mistakes.setdefault(error, (number, row.get('name'), op))
            # A designed operation that names a failure class must end with it.
            if 'expect' in op and not any('failAt' in o for o in row['ops']) and error != op['expect']:
                mistakes.setdefault('missed ' + op['expect'], (number, row.get('name'), op))
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
    missing = [e for e in corpus.NEEDED_EVENTS if e not in events]
    if missing:
        raise RuntimeError(f'the corpus never reaches {missing}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', locale='en-US', unchanged=UNCHANGED,
        constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), scope=SCOPE)
    summary = (f'{len(rows)} original familiar framework cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('familiar fixtures/provenance differ; inspect before regeneration')
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
