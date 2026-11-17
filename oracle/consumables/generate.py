"""Run the unchanged consumable classes on JDK 11 and record what they do.

Compiled unchanged: the nine classes of core/game/consumable (Consumable, Food, Drink, Potion, Cake, HalfableFood,
BarbarianMix, FakeConsumable, ConsumableEffect), content/data/consumables/Consumables.java with every class of
content/data/consumables/effects, content/global/handlers/item/ConsumableListener.kt, core/game/interaction/Script.kt,
Clocks.kt and IntType.kt, core/api/Container.kt, core/game/system/task/Pulse.java and core/tools/TickUtils.kt. The
members named in `markers` are original text inserted into the adapters of adapters.py at generation time. Everything
else is a recording adapter answering from the case. Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/consumables-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/consumables'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('consumables_adapters', HERE / 'adapters.py')
corpus = load('consumables_cases', HERE / 'cases.py')

CORE = ['BarbarianMix', 'Cake', 'Consumable', 'ConsumableEffect', 'Drink', 'FakeConsumable', 'Food', 'HalfableFood', 'Potion']
EFFECTS = sorted(p.name for p in (SERVER / 'content/data/consumables/effects').iterdir() if p.suffix in ('.java', '.kt'))
UNCHANGED = ['core/game/consumable/' + n + '.java' for n in CORE] + \
            ['content/data/consumables/Consumables.java'] + \
            ['content/data/consumables/effects/' + n for n in EFFECTS] + [
    'content/global/handlers/item/ConsumableListener.kt',
    'core/game/interaction/Script.kt',
    'core/game/interaction/Clocks.kt',
    'core/game/interaction/IntType.kt',
    'core/api/Container.kt',
    'core/game/system/task/Pulse.java',
    'core/tools/TickUtils.kt',
]
CONTENT_API = [
    'fun replaceSlot(player: Player, slot: Int, item: Item, currentItem: Item? = null, container: Container = Container.INVENTORY): Item?',
    'fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean',
    'fun getUsedOption(player: Player): String',
    'fun getStatLevel(entity: Entity, skill: Int): Int',
    'fun inInventory(player: Player, id: Int, amount: Int = 1): Boolean',
    'fun modPrayerPoints(player: Player, amount: Double)',
    'fun delayAttack(entity: Entity, ticks: Int)',
    'fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean)',
    'fun stopExecuting(entity: Entity) : Boolean',
    'fun clearScripts(entity: Entity) : Boolean',
    'fun getWorldTicks(): Int',
]
SKILLS = ['public int heal(int health)', 'public void healNoRestrictions(int amount)', 'public int updateLevel(int skill, int amount, int maximum)',
          'public int getStaticLevel(int slot)', 'public int getLevel(int slot, boolean discardAssist)', 'public int getLevel(int slot)',
          'public int getMaximumLifepoints()', 'public int getLifepoints()', 'public double getPrayerPoints()',
          'public void decrementPrayerPoints(double amount)', 'public void incrementPrayerPoints(double amount)', 'public Entity getEntity()']
SCOPE = (
    'Unchanged core/game/consumable (Consumable, Food, Drink, Potion, Cake, HalfableFood, BarbarianMix, FakeConsumable, '
    'ConsumableEffect), content/data/consumables/Consumables.java and every class of content/data/consumables/effects, '
    'content/global/handlers/item/ConsumableListener.kt, Script.kt, Clocks.kt, IntType.kt, core/api/Container.kt, Pulse.java and '
    'TickUtils.kt on JDK 11 with Kotlin 1.8.20, JSON-simple 1.1.1 and the published ConstLib 1.4.28 (Sounds and Items values checked '
    'against the suffixes of the names used). Original text inserted into adapters at generation time: the ContentAPI replaceSlot, '
    'removeItem, getUsedOption, getStatLevel, inInventory, modPrayerPoints, delayAttack, queueScript, stopExecuting, clearScripts and '
    'getWorldTicks functions; Skills\' constants, prayer field and heal, healNoRestrictions, updateLevel, getStaticLevel, both getLevel, '
    'getMaximumLifepoints, getLifepoints, getPrayerPoints, decrementPrayerPoints, incrementPrayerPoints and getEntity; Settings\' run '
    'energy field and updateRunEnergy; RandomFunction.random(a, b), random(max) and nextInt; the Log, HitsplatType, DiaryType and '
    'LogType constants; Item.toString. Everything else is a recording adapter answering from the case: ticks, attributes, animations, audio, '
    'messages (packet dispatch and ContentAPI), manualHit, chat, teleports, pulse manager runs, diary tasks, the familiar and its '
    'special points, inventory/equipment replace, remove, slot remove and contains, the player monitor log, timer spawn/register/'
    'remove/get, the script queue (active script, addToQueue, reset), delayNextAttack, skill packets, the damage-sponge perk, assist '
    'sessions (always absent) and the run energy packet. Item names are the case\'s definition names. RandomFunction.RANDOM is a '
    'seeded recording java.util.Random. ConsumableOptionPlugin is not compiled: it is not @Initializable and its handle body is '
    'commented out.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def markers():
    read = lambda name: c.read(SERVER / name)
    skills = read('core/game/node/entity/skill/Skills.java')
    settings = read('core/game/node/entity/player/link/Settings.java')
    random = read('core/tools/RandomFunction.java')
    api = read('core/api/ContentAPI.kt')
    diary = read('core/game/node/entity/player/link/diary/DiaryType.java')
    monitor = read('core/game/node/entity/player/info/PlayerMonitor.kt')
    diaries = re.findall(r'^(?:\t|    )([A-Z_]+)\("', diary, re.M)
    if diaries[:3] != ['KARAMJA', 'VARROCK', 'LUMBRIDGE']:
        raise RuntimeError('DiaryType constants changed')
    return {
        'ORIGINAL_RANDOM_METHODS': '\n    '.join(c.method(random, s) for s in [
            'public static final int random(int a, int b)', 'public static final int random(int maxValue)', 'public static int nextInt(int val)']),
        'ORIGINAL_LOG_ENUM': c.method(read('core/tools/SystemLogger.kt'), 'enum class Log {'),
        'ORIGINAL_HITSPLAT': c.method(read('core/game/node/entity/combat/ImpactHandler.java'), 'public static enum HitsplatType'),
        'ORIGINAL_SKILL_CONSTANTS': line(skills, 'public static final int ATTACK = 0,'),
        'ORIGINAL_NUM_SKILLS': line(skills, 'public static final int NUM_SKILLS = 24;'),
        'ORIGINAL_PRAYER_FIELD': line(skills, 'private double prayerPoints = 1.;'),
        'ORIGINAL_SKILLS_METHODS': '\n\n    '.join(c.method(skills, s) for s in SKILLS),
        'ORIGINAL_RUN_ENERGY_FIELD': line(settings, 'private double runEnergy = 100.0;'),
        'ORIGINAL_SETTINGS_METHODS': c.method(settings, 'public void updateRunEnergy(double drain)'),
        'ORIGINAL_DIARY_TYPES': ', '.join(diaries),
        'ORIGINAL_LOG_TYPE': c.method(monitor, 'enum class LogType(val token: String) {'),
        'ORIGINAL_CONTENT_API': '\n\n'.join(c.method(api, s) for s in CONTENT_API),
        'ORIGINAL_ITEM_TO_STRING': c.method(read('core/game/node/item/Item.java'), 'public String toString()'),
    }, dict(diaryTypes=diaries)


def libraries():
    """Pinned jars: this run directory first, then any other oracle's run directory, each checked against its pin."""
    found = []
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))['sha256']
        candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
        jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
        if jar is None:
            # Pinned by hash; this generator never downloads.
            raise RuntimeError(f'{name} matching its pin was not found in runs/consumables-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def constants_match_names(classpath):
    """Every Sounds/Items constant the compiled sources name carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    sources = ''.join(c.read(SERVER / name) for name in UNCHANGED)
    checked = {}
    for kind in ['Sounds', 'Items']:
        text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.' + kind], capture_output=True, check=True).stdout.decode()
        names = sorted(set(re.findall(kind + r'\.([A-Z0-9_]+)', sources)))
        for name in names:
            found = re.search(r'int ' + name + r' = (-?\d+);', text)
            if found is None or found.group(1) != name.rsplit('_', 1)[1]:
                raise RuntimeError(f'{kind}.{name} differs from its suffix')
        checked[kind] = names
    return checked


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    runtime = [p for p in jars if p.name.startswith('kotlin-stdlib-')]
    libs = [*runtime, *libraries()]
    kotlin, java = [], []
    for name in UNCHANGED:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted, tables = markers()
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
    c.read(HERE / 'Support.kt')
    kotlin.append(HERE / 'Support.kt')
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    c.run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    checked = constants_match_names(os.pathsep.join(map(str, libs)))
    return lock, classpath, checked, tables


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Support'], input=payload, cwd=ROOT, capture_output=True, timeout=1800)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Cast', 'Index', 'Number', 'Binding')


def generate(check):
    lock, classpath, checked, tables = build()
    planned = list(corpus.cases())
    cases = [case for case, _ in planned]
    rows = execute(classpath, cases)
    # Second pass: in each marked operation every recorded call and draw fails once in turn.
    failing = []
    for (case, sweep), row in zip(planned, rows):
        if sweep is None:
            continue
        calls = len(row['expected'][sweep]['events'])
        for at in range(1, calls + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    rows += execute(classpath, failing)
    cases += failing
    mistakes = {}
    for number, row in enumerate(rows):
        for op, step in zip(row['input']['ops'], row['expected']):
            if step['error'] not in EXPECTED_ERRORS or (step['error'] == 'Binding' and 'failAt' not in op):
                mistakes.setdefault(step['error'], (number, op))
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events, errors = {}, {}, {}
    for row in rows:
        for op, step in zip(row['input']['ops'], row['expected']):
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
        constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), tables=tables, scope=SCOPE)
    summary = (f'{len(rows)} original consumable cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('consumable fixtures/provenance differ; inspect before regeneration')
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
