"""Run the unchanged engine timer classes on JDK 11 and record what they do.

Compiled unchanged: RSTimer, PersistTimer, TimerFlag, TimerManager, TimerRegistry, the fourteen classes of
core/game/system/timer/impl (AntiMacro, Disease, DragonFireImmunity, Frozen, FrozenImmunity, HealOverTime, Miasmic,
MiasmicImmunity, Poison, PoisonImmunity, SkillRestore, Skulled, SpellbookSwap, Teleblock), SpellBookManager, Event,
EventHook, EquipmentSlot, Privilege, Globals and TickUtils. The members named in `markers` are original text
inserted into the adapters of adapters.py at generation time. Everything else is a recording adapter answering from
the case. Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/content-timers-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/content-timers'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('content_timers_adapters', HERE / 'adapters.py')
corpus = load('content_timers_cases', HERE / 'cases.py')

TIMER = 'core/game/system/timer/'
CLASSES = ['AntiMacro', 'Disease', 'DragonFireImmunity', 'Frozen', 'FrozenImmunity', 'HealOverTime', 'Miasmic',
           'MiasmicImmunity', 'Poison', 'PoisonImmunity', 'SkillRestore', 'Skulled', 'SpellbookSwap', 'Teleblock']
UNCHANGED = [TIMER + n + '.kt' for n in ['RSTimer', 'PersistTimer', 'TimerFlag', 'TimerManager', 'TimerRegistry']] + \
            [TIMER + 'impl/' + n + '.kt' for n in CLASSES] + [
    'core/game/node/entity/player/link/SpellBookManager.java',
    'core/game/event/Event.kt',
    'core/game/event/EventHook.kt',
    'core/api/EquipmentSlot.kt',
    'core/game/system/command/Privilege.kt',
    'core/tools/Globals.kt',
    'core/tools/TickUtils.kt',
]
CONTENT_API = [
    'fun registerTimer (entity: Entity, timer: RSTimer?)',
    'fun getOrStartTimer (entity: Entity, identifier: String, vararg args: Any) : RSTimer?',
    'inline fun <reified T: RSTimer> getOrStartTimer (entity: Entity, vararg args: Any) : T',
    'fun spawnTimer (identifier: String, vararg args: Any) : RSTimer?',
    'inline fun <reified T: RSTimer> spawnTimer (vararg args: Any) : T',
    'inline fun <reified T: RSTimer> hasTimerActive (entity: Entity) : Boolean',
    'inline fun <reified T: RSTimer> getTimer (entity: Entity) : T?',
    'inline fun <reified T: RSTimer> removeTimer (entity: Entity)',
    'fun hasTimerActive (entity: Entity, identifier: String) : Boolean',
    'fun getTimer (entity: Entity, identifier: String) : RSTimer?',
    'fun removeTimer (entity: Entity, identifier: String)',
    'fun removeTimer (entity: Entity, timer: RSTimer)',
    'fun getStatLevel(entity: Entity, skill: Int): Int',
    'fun getDynLevel(entity: Entity, skill: Int): Int',
    'fun getItemFromEquipment(player: Player, slot: EquipmentSlot): Item?',
    'fun applyPoison (entity: Entity, source: Entity, severity: Int)',
    'fun isPoisoned (entity: Entity) : Boolean',
    'fun curePoison (entity: Entity)',
    'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T',
    'fun <T> setAttribute(entity: Entity, attribute: String, value: T)',
    'fun removeAttribute(entity: Entity, attribute: String)',
]
SKILLS = ['public int heal(int health)', 'public int updateLevel(int skill, int amount, int maximum)', 'public int getStaticLevel(int slot)',
          'public int getLevel(int slot, boolean discardAssist)', 'public int getLevel(int slot)', 'public int getMaximumLifepoints()',
          'public int getLifepoints()', 'public Entity getEntity()']
HOOKS = {'public void dispatch(Event event)': 'private void originalDispatch(Event event)',
         'public void unhook(EventHook hook)': 'private void originalUnhook(EventHook hook)',
         'public void hook(Class<?> event, EventHook hook)': 'private void originalHook(Class<?> event, EventHook hook)'}
EVENTS = ['DialogueCloseEvent', 'SpellCastEvent', 'SpellbookChangeEvent', 'PrayerActivatedEvent', 'PrayerDeactivatedEvent']
EVENT_CONSTANTS = ['DialogueClosed', 'SpellCast', 'SpellbookChanged', 'PrayerActivated', 'PrayerDeactivated']
SCOPE = (
    'Unchanged RSTimer, PersistTimer, TimerFlag, TimerManager, TimerRegistry and the fourteen engine timers of '
    'core/game/system/timer/impl (AntiMacro, Disease, DragonFireImmunity, Frozen, FrozenImmunity, HealOverTime, Miasmic, '
    'MiasmicImmunity, Poison, PoisonImmunity, SkillRestore, Skulled, SpellbookSwap, Teleblock), registered once each in the '
    'class scan\'s order through their own constructors, with the unchanged SpellBookManager, Event, EventHook, EquipmentSlot, '
    'Privilege, Globals and TickUtils, on JDK 11 with Kotlin 1.8.20, kotlin-reflect 1.6.10 (createInstance), JSON-simple 1.1.1 and the '
    'published ConstLib 1.4.28 (Sounds and Items values checked against the suffixes of the names used). Original text inserted into '
    'adapters at generation time: the ContentAPI timer, poison, level, equipment and attribute functions; Entity\'s hook map with its '
    'dispatch, unhook and hook bodies; Skills\' constants, lastTrainedSkill/lastXpGain and heal, updateLevel, getStaticLevel, getLevel, '
    'getMaximumLifepoints, getLifepoints and getEntity; Settings\' special energy field, getter and setter; Player\'s logout listener map; '
    'RandomFunction.random(a, b) and random(max); the Log, Rights, PrayerType, ZoneRestriction and HitsplatType constants; '
    'Location.toString; the five hooked event declarations and their core.api.Event constants; the RandomEvents constant names '
    'with their loot and type arguments. Everything else is a recording adapter answering from the case: ticks, names, locations, '
    'script delay and weak scripts, modal state, debug messages, rights, artificial flag, uid lookups, slayer tasks, familiar owners, '
    'manualHit, skill packets, the damage-sponge perk, assist sessions (always absent), equipment, varps and varbits, messages, audio, '
    'skull icon and state, tabs, the weapon interface extension and its selectAutoSpell, autocast spell presence, attributes, '
    'poison immunity, zone restriction, dialogue ids, logout listener registration, logging, the random event pool, event NPC '
    'creation, spawn location, init, terminate and loot. RandomFunction.RANDOM is a seeded recording java.util.Random. Printed stack '
    'traces are recorded by exception class. Not driven: AntiMacro.defineCommands (the ::revent command), the random event content, '
    'farming/other content timers, the world tick loop and Entity.init/finalizeDeath.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def random_events(text):
    body = text[text.index('enum class RandomEvents('):]
    body = body[body.index('{') + 1:body.index('companion object')]
    starts = [(m.start(), m.group(1)) for m in re.finditer(r'^    ([A-Z_]+)\(', body, re.M)]
    entries, table = [], []
    for i, (at, name) in enumerate(starts):
        segment = body[at:starts[i + 1][0] if i + 1 < len(starts) else len(body)]
        if 'loot = WeightBasedTable.create(' in segment:
            loot, loot_label = f'WeightBasedTable("{name}")', name
        else:
            shared = re.search(r'loot = ([A-Z_]+)\.loot', segment)
            loot, loot_label = (f'{shared.group(1)}.loot', shared.group(1)) if shared else ('null', None)
        kind = re.search(r'type = "([^"]*)"', segment)
        kind = kind.group(1) if kind else ''
        entries.append(f'{name}(RandomEventNPC("{name}"), {loot}, "{kind}")')
        table.append([name, loot_label, kind])
    if [n for n, _, _ in table][:3] != ['SANDWICH_LADY', 'GENIE', 'CANDLELIGHT']:
        raise RuntimeError('RandomEvents constants changed')
    return ',\n    '.join(entries), table


def markers():
    read = lambda name: c.read(SERVER / name)
    entity = read('core/game/node/entity/Entity.java')
    player = read('core/game/node/entity/player/Player.java')
    skills = read('core/game/node/entity/skill/Skills.java')
    settings = read('core/game/node/entity/player/link/Settings.java')
    random = read('core/tools/RandomFunction.java')
    api = read('core/api/ContentAPI.kt')
    events = read('core/game/event/Events.kt')
    constants = read('core/api/Event.kt')
    prayer = read('core/game/node/entity/player/link/prayer/PrayerType.java')
    zones = read('core/game/world/map/zone/ZoneRestriction.java')
    rights = line(read('core/game/node/entity/player/info/Rights.java'), 'REGULAR_PLAYER,')
    entries, table = random_events(read('content/global/ame/RandomEvents.kt'))
    hooks = []
    for signature, renamed in HOOKS.items():
        hooks.append(c.method(entity, signature).replace(signature, renamed))
    zone_names = re.findall(r'^\t([A-Z_]+)\s*[,;(]', zones, re.M)
    if 'RANDOM_EVENTS' not in zone_names:
        raise RuntimeError('ZoneRestriction constants changed')
    prayers = re.findall(r'^\t([A-Z_]+)\(', prayer, re.M)
    return {
        'ORIGINAL_RANDOM_METHODS': '\n    '.join(c.method(random, s) for s in [
            'public static final int random(int a, int b)', 'public static final int random(int maxValue)']),
        'ORIGINAL_LOG_ENUM': c.method(read('core/tools/SystemLogger.kt'), 'enum class Log {'),
        'ORIGINAL_LOCATION_TO_STRING': line(read('core/game/world/map/Location.kt'), 'override fun toString(): String = "[$x, $y, $z]"'),
        'ORIGINAL_HOOKS_FIELD': line(entity, 'private HashMap<Class<?>, ArrayList<EventHook>> hooks'),
        'ORIGINAL_HOOK_METHODS': '\n\n    '.join(hooks),
        'ORIGINAL_HITSPLAT': c.method(read('core/game/node/entity/combat/ImpactHandler.java'), 'public static enum HitsplatType'),
        'ORIGINAL_SKILL_CONSTANTS': line(skills, 'public static final int ATTACK = 0,'),
        'ORIGINAL_SKILL_FIELDS': '\n    '.join([line(skills, 'public int lastTrainedSkill = -1;'), line(skills, 'public int lastXpGain = 0;')]),
        'ORIGINAL_SKILLS_METHODS': '\n\n    '.join(c.method(skills, s) for s in SKILLS),
        'ORIGINAL_LOGOUT_LISTENERS': line(player, 'public HashMap<String,Function1<Player, Unit>> logoutListeners'),
        'ORIGINAL_RIGHTS': ', '.join(re.findall(r'[A-Z_]+', rights)),
        'ORIGINAL_SPECIAL_FIELD': line(settings, 'private int specialEnergy = 100;'),
        'ORIGINAL_SETTINGS_METHODS': '\n\n    '.join(c.method(settings, s) for s in [
            'public void setSpecialEnergy(int value)', 'public int getSpecialEnergy()']),
        'ORIGINAL_PRAYER_TYPES': ', '.join(prayers),
        'ORIGINAL_ZONE_RESTRICTIONS': ', '.join(zone_names),
        'ORIGINAL_EVENTS': '\n'.join(line(events, f'data class {e}') for e in EVENTS),
        'ORIGINAL_EVENT_CONSTANTS': '\n    '.join(line(constants, f'@JvmStatic val {e} =') for e in EVENT_CONSTANTS),
        'ORIGINAL_RANDOM_EVENTS': entries,
        'ORIGINAL_CONTENT_API': '\n\n'.join(c.method(api, s) for s in CONTENT_API),
    }, dict(randomEvents=table, prayerTypes=prayers, zoneRestrictions=zone_names)


def libraries():
    """Pinned jars: this run directory first, then any other oracle's run directory, each checked against its pin."""
    found = []
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))['sha256']
        candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
        jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
        if jar is None:
            # Pinned by hash; this generator never downloads.
            raise RuntimeError(f'{name} matching its pin was not found in runs/content-timers-oracle or another runs/*-oracle directory')
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
    runtime = [p for p in jars if p.name.startswith(('kotlin-stdlib-', 'kotlin-reflect-'))]
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


EXPECTED_ERRORS = (None, 'Null', 'Uninitialized', 'Number', 'Cast', 'Index', 'Binding')


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
        calls = len([e for e in row['expected'][sweep]['events'] if e[0] != 'exception'])
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
            for event in step['events']:
                if event[0] == 'exception' and event[1] not in EXPECTED_ERRORS:
                    mistakes.setdefault(event[1], (number, op))
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
    summary = (f'{len(rows)} original engine timer cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('content timer fixtures/provenance differ; inspect before regeneration')
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
