"""Run the unchanged prayer classes on JDK 11 and record what they do.

Compiled unchanged: Prayer.java, PrayerType.java, PrayerCategory.java,
SkillBonus.java, Audio.kt and PrayerTabInterface.kt. The members named in
`markers` are original text inserted into the adapters of adapters.py at
generation time. Everything else is a recording adapter answering from the
case. Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/prayer-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/prayer'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('prayer_adapters', HERE / 'adapters.py')
corpus = load('prayer_cases', HERE / 'cases.py')

PRAYER = 'core/game/node/entity/player/link/prayer/'
UNCHANGED = [
    PRAYER + 'Prayer.java',
    PRAYER + 'PrayerType.java',
    PRAYER + 'PrayerCategory.java',
    'core/game/node/entity/skill/SkillBonus.java',
    'core/game/node/entity/player/link/audio/Audio.kt',
    'content/global/handlers/iface/tabs/PrayerTabInterface.kt',
]
SKILLS = ['public int getStaticLevel(int slot)', 'public double getPrayerPoints()', 'public void decrementPrayerPoints(double amount)',
          'public void setPrayerPoints(double amount)', 'public int heal(int health)', 'public int getMaximumLifepoints()']
PLAYER = ['public boolean hasProtectionPrayer(CombatStyle style)', 'public void commenceDeath(Entity killer)', 'public void fullRestore()']
SCOPE = (
    'Unchanged Prayer, PrayerType, PrayerCategory, SkillBonus, Audio and PrayerTabInterface on JDK 11 with Kotlin 1.8.20 and the '
    'published ConstLib 1.4.28 (Sounds and Components; the bundled ConstLib-1.4.jar lacks the prayer sound names). Original text '
    'inserted into adapters at generation time: Skills.getStaticLevel, getPrayerPoints, decrementPrayerPoints, setPrayerPoints, heal, '
    'getMaximumLifepoints and the skill constants; Player.hasProtectionPrayer, commenceDeath and fullRestore; NPC.hasProtectionPrayer; '
    'Entity.getFormattedHit and isIgnoreProtection; ContentAPI hasLevelStat and setVarp; StringUtils.formatDisplayName; RandomFunction '
    'random and randomize; the Location packing, transform, withinDistance and getDistance; the ImpactHandler.HitsplatType enum; the '
    'CombatStyle protection prayers and ArmourSet constant names; the PrayerActivatedEvent/PrayerDeactivatedEvent declarations. '
    'Everything else is a recording adapter answering from the case: dispatch, appearance head icon and sync, audio, dialogue, '
    'King\'s Ransom requirement, varbit 3909, the pulse runner (queued pulses fire when the case asks), messages, bonuses, skillcape '
    'perks, graphics, locations, projectiles, manualHit, multi-zone, local entities, isAttackable, isPlaying, the superclass '
    'commenceDeath/fullRestore and the two Settings energy setters. RandomFunction.RANDOM is a seeded java.util.Random. Not driven: '
    'prayer training (bones, altars), SkillRestore\'s event hooks, DeathTask.getContainers (PROTECT_ITEMS), getDragonfireProtection, '
    'finalizeDeath, reentrant dispatch hooks and the real CombatSwingHandler/ImpactHandler (their own corpora drive them).'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def markers():
    read = lambda name: c.read(SERVER / name)
    entity = read('core/game/node/entity/Entity.java')
    npc = read('core/game/node/entity/npc/NPC.java')
    player = read('core/game/node/entity/player/Player.java')
    skills = read('core/game/node/entity/skill/Skills.java')
    location = read('core/game/world/map/Location.kt')
    random = read('core/tools/RandomFunction.java')
    api = read('core/api/ContentAPI.kt')
    style = read('core/game/node/entity/combat/CombatStyle.java')
    armour = read('core/game/node/entity/combat/equipment/ArmourSet.java')
    events = read('core/game/event/Events.kt')
    within = c.method(location, '@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)')
    styles = re.findall(r'^\t([A-Z]+)\(new \w+\(\), (PrayerType\.\w+)\)', style, re.M)
    if [s for s, _ in styles] != ['MELEE', 'RANGE', 'MAGIC']:
        raise RuntimeError('CombatStyle constants changed')
    return {
        'ORIGINAL_RANDOM_METHODS': '\n'.join(c.method(random, s) for s in [
            'public static final int random(int maxValue)', 'public static int randomize(int value)']),
        'ORIGINAL_FORMAT_DISPLAY_NAME': c.method(read('core/tools/StringUtils.java'), 'public static String formatDisplayName(String name)'),
        'ORIGINAL_LOCATION_MEMBERS': '\n    '.join([
            line(location, 'fun transform(diffX: Int, diffY: Int, z: Int)'),
            within.replace('@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)',
                           'fun withinDistance(other: Location, dist: Int)')]),
        'ORIGINAL_GET_DISTANCE': c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location)'),
        'ORIGINAL_LOCATION_HASH': c.method(location, 'value class LocationHash('),
        'ORIGINAL_COMBAT_STYLES': ', '.join(f'{s}({p})' for s, p in styles),
        'ORIGINAL_ARMOUR_SETS': ', '.join(re.findall(r'^\t([A-Z_]+)\(', armour, re.M)),
        'ORIGINAL_HITSPLAT': c.method(read('core/game/node/entity/combat/ImpactHandler.java'), 'public static enum HitsplatType'),
        'ORIGINAL_ENTITY_METHODS': '\n\n'.join(c.method(entity, s) for s in [
            'public double getFormattedHit(BattleState state, int hit)', 'public boolean isIgnoreProtection(CombatStyle style)']),
        'ORIGINAL_NPC_METHODS': c.method(npc, 'public boolean hasProtectionPrayer(CombatStyle style)'),
        'ORIGINAL_PLAYER_METHODS': '\n\n'.join(c.method(player, s) for s in PLAYER),
        'ORIGINAL_SKILL_CONSTANTS': line(skills, 'public static final int ATTACK = 0,'),
        'ORIGINAL_SKILLS_METHODS': '\n\n'.join(c.method(skills, s) for s in SKILLS),
        'ORIGINAL_HAS_LEVEL_STAT': c.method(api, 'fun hasLevelStat(player: Player, skill: Int, level: Int): Boolean'),
        'ORIGINAL_SET_VARP': '@JvmOverloads\n' + c.method(api, 'fun setVarp (player: Player, varpIndex: Int, value: Int, save: Boolean = false)'),
        'ORIGINAL_PRAYER_EVENTS': '\n'.join([line(events, 'data class PrayerActivatedEvent'), line(events, 'data class PrayerDeactivatedEvent')]),
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/prayer-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def sounds_match_names(classpath):
    """Every Sounds constant PrayerType and Prayer name carries its id as its suffix; check the jar's values agree."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    text = subprocess.run([str(javap), '-cp', classpath, '-constants', 'org.rs09.consts.Sounds'], capture_output=True, check=True).stdout.decode()
    sources = c.read(SERVER / (PRAYER + 'PrayerType.java')) + c.read(SERVER / (PRAYER + 'Prayer.java'))
    names = sorted(set(re.findall(r'Sounds\.([A-Z0-9_]+)', sources)))
    for name in names:
        found = re.search(r'int ' + name + r' = (-?\d+);', text)
        if found is None or found.group(1) != name.rsplit('_', 1)[1]:
            raise RuntimeError(f'Sounds.{name} differs from its suffix')
    return names


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        reference = ROOT / 'reference' / part
        if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock[part]['commit']:
            raise RuntimeError('Reference pin differs')
        c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    libs = [stdlib, *libraries()]
    kotlin, java = [], []
    for name in UNCHANGED:
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
           '-jvm-target', '11', '-nowarn', '-classpath', os.pathsep.join(map(str, libs)), '-d', classes, *kotlin, *java])
    classpath = os.pathsep.join(map(str, [classes, *libs]))
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-cp', classpath, '-d', classes, *java])
    sounds = sounds_match_names(os.pathsep.join(map(str, libs)))
    return lock, classpath, sounds


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Support'], input=payload, cwd=ROOT, capture_output=True, timeout=900)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    return rows


def generate(check):
    lock, classpath, sounds = build()
    planned = corpus.cases()
    cases = [case for case, _ in planned]
    rows = execute(classpath, cases)
    # Second pass: in each marked operation every recorded call and draw fails once in turn.
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
        for op, step in zip(row['input']['ops'], row['expected']):
            if step['error'] not in (None, 'Null', 'Index', 'Binding') or (step['error'] == 'Binding' and 'failAt' not in op):
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
    for needed in ['send_varp', 'dispatch', 'play_audio', 'set_head_icon', 'head_icon', 'sync', 'send_dialogue', 'has_requirement',
                   'get_varbit', 'set_varbit', 'submit', 'send_message', 'bonuses', 'perk', 'next_int', 'skill_packet', 'graphics',
                   'location', 'projectile', 'manual_hit', 'multi_zone', 'local_npcs', 'local_players', 'is_attackable', 'is_playing',
                   'commence_death_super', 'set_special_energy', 'update_run_energy', 'full_restore_super']:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
        coverage=coverage, sourceHashAlgorithm='sha256, CR removed', kotlinCompiler='1.8.20', locale='en-US', unchanged=UNCHANGED,
        constantsJar=dict(lock='oracle/bank/constants.lock.json', soundsCheckedAgainstNames=sounds), scope=SCOPE)
    summary = (f'{len(rows)} original prayer cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('prayer fixtures/provenance differ; inspect before regeneration')
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
