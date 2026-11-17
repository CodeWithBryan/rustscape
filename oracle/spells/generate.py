"""Run the unchanged spell framework and combat spell classes on JDK 11 and record what they do.

Compiled unchanged: MagicSpell, CombatSpell, DefaultCombatSpell, SpellType, Runes, CombinationRune, MagicStaff,
SpellBlocks, SpellBookManager, Audio, SpellUtils and the seventeen combat spell classes of the standard and ancient
books. The members named in `markers` are original text inserted into the adapters of adapters.py at generation
time. Everything else is a recording adapter answering from the case. Java/Kotlin are test oracles only; --check
never rewrites a fixture.
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
RUN = ROOT / 'runs/spells-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/spells'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
adapters = load('spells_adapters', HERE / 'adapters.py')
corpus = load('spells_cases', HERE / 'cases.py')

SPELL = 'core/game/node/entity/combat/spell/'
MAGIC = 'content/global/skill/magic/'
CONTENT = [MAGIC + 'modern/' + n + '.java' for n in ['AirSpell', 'WaterSpell', 'EarthSpell', 'FireSpell', 'BindSpell', 'CurseSpells',
                                                      'GodSpells', 'CrumbleUndead', 'IbanBlast', 'MagicDart', 'TeleblockSpell',
                                                      'SonicWaveSpell']] + \
          [MAGIC + 'ancient/' + n + '.java' for n in ['IceSpells', 'BloodSpells', 'ShadowSpells', 'SmokeSpells', 'MiasmicSpells']]
UNCHANGED = [SPELL + n + '.java' for n in ['MagicSpell', 'CombatSpell', 'DefaultCombatSpell', 'SpellType', 'Runes', 'CombinationRune',
                                           'MagicStaff', 'SpellBlocks']] + [
    'core/game/node/entity/player/link/SpellBookManager.java',
    'core/game/node/entity/player/link/audio/Audio.kt',
    MAGIC + 'SpellUtils.kt',
] + CONTENT
SCOPE = (
    'Unchanged MagicSpell, CombatSpell, DefaultCombatSpell, SpellType, Runes, CombinationRune, MagicStaff, SpellBlocks, '
    'SpellBookManager, Audio and SpellUtils, and the unchanged combat spell classes AirSpell, WaterSpell, EarthSpell, FireSpell, '
    'BindSpell, CurseSpells, GodSpells, CrumbleUndead, IbanBlast, MagicDart, TeleblockSpell, SonicWaveSpell, IceSpells, BloodSpells, '
    'ShadowSpells, SmokeSpells and MiasmicSpells (registered once through their own newInstance), on JDK 11 with Kotlin 1.8.20 and the '
    'published ConstLib 1.4.28 (Sounds, Items, NPCs and Components values checked against the suffixes of the names used). Original '
    'text inserted into adapters at generation time: RandomFunction.getRandom, the Location packing, withinDistance and getDistance, '
    'the Animator.Priority constants, the EquipmentContainer slot and Skills constants, the SpellCastEvent declaration, the Skogre and '
    'Zogre id arrays, the ContentAPI getAttribute/setAttribute/removeAttribute/removeAttributes functions, and WeaponInterface\'s four '
    'autocast spell tables, style constants, autocast attribute keys and getAutospellId, selectAutoSpell and canAutocast. Math.random\'s '
    'generator is replaced by the case\'s scripted or seeded one. Everything else is a recording adapter answering from the case: '
    'ticks, development mode, entity names, locations, levels, experience grants, worn items, inventory amounts and a stand-in '
    'inventory removal, attributes (a map with GameAttributes\' /save: key handling), messages, interface configs, varbits, tabs, '
    'component visibility, facing, graphics, animations, global audio, projectiles (transform/send recorded with the template\'s field '
    'name; the projectile engine is not run), the combat pulse members CombatSpell.cast calls, autocast/spell properties, attack styles, '
    'zones, timers, god casts, slayer tasks, NPC combat graphics and multi-target lists. Not driven: the spells\' fireEffect bodies '
    '(freeze, drain, heal, poison, bind, teleblock and miasmic timers), Projectile internals, the real CombatPulse/MagicSwingHandler, '
    'Container internals, the non-combat spells and every other spell book registration.'
)


def line(text, prefix):
    for candidate in text.split('\n'):
        if candidate.strip().startswith(prefix):
            return candidate.strip()
    raise RuntimeError(f'no line starts with {prefix}')


def lines(text, prefix):
    found = [candidate.strip() for candidate in text.split('\n') if candidate.strip().startswith(prefix)]
    if not found:
        raise RuntimeError(f'no line starts with {prefix}')
    return '\n    '.join(found)


def markers():
    read = lambda name: c.read(SERVER / name)
    location = read('core/game/world/map/Location.kt')
    random = read('core/tools/RandomFunction.java')
    animator = read('core/game/node/entity/impl/Animator.java')
    weapon = read('core/game/node/entity/combat/equipment/WeaponInterface.java')
    api = read('core/api/ContentAPI.kt')
    within = c.method(location, '@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)')
    body = animator[animator.index('public static enum Priority {'):]
    body = body[:body.index('}')]
    priorities = re.findall(r'^\t\t([A-Z_]+)[,;]', body, re.M)
    if priorities != ['LOW', 'MID', 'HIGH', 'VERY_HIGH']:
        raise RuntimeError(f'Animator.Priority changed: {priorities}')
    zogre = read('content/region/kandarin/feldip/quest/zogreflesheaters/ZogreBehavior.kt')
    skogre = read('content/region/kandarin/feldip/quest/zogreflesheaters/SkogreBehavior.kt')
    return {
        'ORIGINAL_GET_RANDOM': c.method(random, 'public static final int getRandom(int maxValue)'),
        'ORIGINAL_WITHIN_DISTANCE': within.replace('@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)',
                                                   'fun withinDistance(other: Location, dist: Int)'),
        'ORIGINAL_GET_DISTANCE': c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location)'),
        'ORIGINAL_LOCATION_HASH': c.method(location, 'value class LocationHash('),
        'ORIGINAL_PRIORITIES': ', '.join(priorities),
        'ORIGINAL_SLOTS': line(read('core/game/container/impl/EquipmentContainer.java'), 'public static final int SLOT_HAT = 0,'),
        'ORIGINAL_SKILL_CONSTANTS': line(read('core/game/node/entity/skill/Skills.java'), 'public static final int ATTACK = 0,'),
        'ORIGINAL_SPELL_TABLES': lines(weapon, 'private static final int[] ') ,
        'ORIGINAL_STYLE_CONSTANTS': lines(weapon, 'public static final int STYLE_'),
        'ORIGINAL_AUTOCAST_KEYS': lines(weapon, 'public static final String ATTRIBUTE_AUTOCAST_'),
        'ORIGINAL_AUTOCAST_METHODS': '\n\n    '.join(c.method(weapon, s) for s in [
            'public int getAutospellId(int spellId)', 'public void selectAutoSpell(int buttonId, boolean adjustAttackStyle, boolean forgetAutocast)',
            'public boolean canAutocast(boolean message)']),
        'ORIGINAL_SPELL_CAST_EVENT': line(read('core/game/event/Events.kt'), 'data class SpellCastEvent'),
        'ORIGINAL_SKOGRE_IDS': c.method(skogre, 'companion object {'),
        'ORIGINAL_ZOGRE_IDS': c.method(zogre, 'companion object {'),
        'ORIGINAL_ATTRIBUTE_FUNCTIONS': '\n\n'.join(c.method(api, s) for s in [
            'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T', 'fun <T> setAttribute(entity: Entity, attribute: String, value: T)',
            'fun removeAttribute(entity: Entity, attribute: String)', 'fun removeAttributes(entity: Entity, vararg attributes: String)']),
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
            raise RuntimeError(f'{name} matching its pin was not found in runs/spells-oracle or another runs/*-oracle directory')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def constants_match_names(classpath):
    """Every Sounds/Items/NPCs/Components constant the compiled sources name carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    sources = ''.join(c.read(SERVER / name) for name in UNCHANGED)
    sources += c.read(SERVER / 'core/game/node/entity/combat/equipment/WeaponInterface.java')
    sources += c.read(SERVER / 'content/region/kandarin/feldip/quest/zogreflesheaters/ZogreBehavior.kt')
    sources += c.read(SERVER / 'content/region/kandarin/feldip/quest/zogreflesheaters/SkogreBehavior.kt')
    checked = {}
    for kind in ['Sounds', 'Items', 'NPCs', 'Components']:
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
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-XDignore.symbol.file', '-cp', classpath, '-d', classes, *java])
    checked = constants_match_names(os.pathsep.join(map(str, libs)))
    (RUN / 'names.json').write_text(json.dumps(corpus.NAMES, sort_keys=True), encoding='utf-8', newline='\n')
    return lock, classpath, checked


def execute(classpath, cases):
    executable = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    result = subprocess.run([str(executable), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', classpath,
                             'oracle.Support', str(RUN / 'names.json')], input=payload, cwd=ROOT, capture_output=True, timeout=900)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace') + result.stdout.decode(errors='replace')[-2000:])
    rows = [json.loads(text) for text in result.stdout.splitlines() if text]
    if len(rows) != len(cases):
        raise RuntimeError(f'{len(rows)} results for {len(cases)} cases')
    return rows


EXPECTED_ERRORS = (None, 'Null', 'Index', 'Cast', 'Binding')


def generate(check):
    lock, classpath, checked = build()
    planned = list(corpus.cases())
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
        constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked), itemNames=corpus.NAMES, scope=SCOPE)
    summary = (f'{len(rows)} original spell cases ({coverage["actions"]} actions, {coverage["recordedCalls"]} recorded calls, '
               f'{len(failing)} failure cases)')
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('spell fixtures/provenance differ; inspect before regeneration')
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
