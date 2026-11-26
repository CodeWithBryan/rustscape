"""Run the unchanged book interface and the five global books on JDK 11 and record what they do.

Compiled unchanged: content/global/handlers/iface/BookInterface.kt and GeneralRuleBook, GoblinBook, GrimDiary,
SecurityBookPlugin and StrongholdNotes (with the unchanged IntType). Original text inserted at generation time:
ContentAPI's getAttribute, setAttribute, openInterface and closeInterface. Everything else is a recording adapter
(adapters.py): the InteractionListener and InterfaceListener registries, the player's attributes, interface manager
and packet dispatch, storeBookInHouse, GameWorld's settings and ServerConstants. Every case runs in a class loader of
its own (oracle/global-items2/Boot.java), so each book class initialises with the case's names. --check never
rewrites a fixture.
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
RUN = ROOT / 'runs/books-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/books'
BOOT = ROOT / 'oracle/global-items2/Boot.java'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('books_adapters', HERE / 'adapters.py')
corpus = load('books_cases', HERE / 'cases.py')

B = 'content/global/handlers/item/book/'
UNCHANGED = [
    'content/global/handlers/iface/BookInterface.kt',
    B + 'GeneralRuleBook.kt',
    B + 'GoblinBook.kt',
    B + 'GrimDiary.kt',
    B + 'SecurityBookPlugin.kt',
    B + 'StrongholdNotes.kt',
    'core/game/interaction/IntType.kt',
]
SCOPE = (
    'The unchanged BookInterface and GeneralRuleBook, GoblinBook, GrimDiary, SecurityBookPlugin and StrongholdNotes on JDK 11 with '
    'Kotlin 1.8.20 and the published ConstLib 1.4.28 (Items values checked against the suffixes of the names used), with the '
    'unchanged IntType. Original text inserted: ContentAPI getAttribute, setAttribute, openInterface and closeInterface. Recording '
    'adapters: the InteractionListener and InterfaceListener registries (registration recorded in order with the lambda classes), '
    'the player attributes, interface manager and packet dispatch, storeBookInHouse, GameWorld.settings and ServerConstants. Every '
    'case runs in a class loader of its own; the k-th recorded call of a marked operation fails in the injected failure cases.'
)


def markers():
    api = c.read(SERVER / 'core/api/ContentAPI.kt')
    return {
        'ORIGINAL_GET_ATTRIBUTE': c.method(api, 'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T {'),
        'ORIGINAL_SET_ATTRIBUTE': c.method(api, 'fun <T> setAttribute(entity: Entity, attribute: String, value: T) {'),
        'ORIGINAL_OPEN_INTERFACE': c.method(api, 'fun openInterface(player: Player, id: Int) {'),
        'ORIGINAL_CLOSE_INTERFACE': c.method(api, 'fun closeInterface(player: Player) {'),
    }


def constants_match_names(classpath, texts):
    """Every ConstLib Items constant the sources name carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.Items'], capture_output=True, check=True).stdout.decode()
    used = sorted(set(re.findall(r'Items\.([A-Z][A-Z0-9_]*_\d+)\b', ''.join(texts))))
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
    libs = [stdlib]
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))['sha256']
        jar = RUN / name
        # Pinned by hash; this generator never downloads. Place the artifact here from another oracle's run directory.
        if not jar.is_file() or hashlib.sha256(jar.read_bytes()).hexdigest() != pin:
            raise RuntimeError(f'{jar} is missing or differs from its pin')
        libs.append(jar)
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
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'adapters.py')
    c.read(HERE / 'cases.py')
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


def generate(check):
    lock, classpath, classes, checked = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, classes, cases)
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        for at in range(1, len(row['expected'][sweep]['events']) + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, name=f"{case['name']} / failing call {at}", ops=ops))
    rows += execute(classpath, classes, failing)
    cases += failing
    (RUN / 'last.jsonl').write_text(''.join(json.dumps(row, sort_keys=True) + '\n' for row in rows), encoding='utf-8')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
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
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED,
        constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), scope=SCOPE)
    summary = f'{len(rows)} original book cases ({coverage["actions"]} actions, {len(failing)} failure cases)'
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('book fixtures/provenance differ; inspect before regeneration')
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
