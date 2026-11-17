"""Reproduce what the original combat code does around and after a swing.

CombatPulse, ImpactHandler, Pulse, the four swing handlers, BattleState and the
equipment data classes they read are compiled unmodified on JDK 11, with the
projectile pathfinder. Everything they reach outside that slice is a recording
stand-in: oracle/combat-swing/adapters.txt first, then this directory's
adapters.txt replacing or adding files. Java/Kotlin are test oracles only;
--check never rewrites fixtures.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
RUN = ROOT / 'runs/combat-pulse-oracle'
SOURCE = ROOT / 'reference/server/Server/src/main'
COMBAT = 'core/game/node/entity/combat/'
WORLD = 'core/game/world/map/'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
corpus = load('combat_pulse_corpus', HERE / 'corpus.py')

UNMODIFIED = [
    COMBAT + 'CombatSwingHandler.kt', COMBAT + 'MeleeSwingHandler.kt', COMBAT + 'RangeSwingHandler.kt',
    COMBAT + 'MagicSwingHandler.kt', COMBAT + 'BattleState.java', COMBAT + 'CombatStyle.java',
    COMBAT + 'InteractionType.java', COMBAT + 'CombatPulse.kt', COMBAT + 'ImpactHandler.java',
    COMBAT + 'equipment/Weapon.java', COMBAT + 'equipment/RangeWeapon.java',
    COMBAT + 'equipment/Ammunition.java', COMBAT + 'equipment/ArmourSet.java', COMBAT + 'equipment/BoltEffect.java',
    'core/api/EquipmentSlot.kt', 'core/game/system/task/Pulse.java', WORLD + 'Direction.java', WORLD + 'Point.java',
    WORLD + 'path/Path.java', WORLD + 'path/ProjectilePathfinder.java', WORLD + 'path/ClipMaskSupplier.java',
    WORLD + 'zone/ZoneType.java',
]
# Stand-ins of the swing corpus that an unmodified class or a generated file replaces here.
DROPPED = [
    'core/game/world/GameWorld.java', 'core/game/system/task/Pulse.java', WORLD + 'Direction.java',
    WORLD + 'path/ClipMaskSupplier.java', WORLD + 'path/Path.java', WORLD + 'path/Pathfinder.java',
    COMBAT + 'ImpactHandler.java',
]


def constants_jar(read, run, lock):
    """The pinned bundled constants jar, taken from the reference object store."""
    evidence = json.loads(read(ROOT / 'oracle/pickup/consts-evidence.json'))
    blob = run(['git', '-C', ROOT / 'reference/server', 'show', lock['server']['commit'] + ':Server/libs/ConstLib-1.4.jar']).stdout
    assert len(blob) == evidence['bytes'] and hashlib.sha256(blob).hexdigest() == evidence['sha256']
    jar = RUN / 'ConstLib-1.4.jar'
    if not jar.is_file() or jar.read_bytes() != blob:
        jar.write_bytes(blob)
    return jar, evidence['sha256']


def split(text):
    parts = re.split(r'^//// (\S+)\n', text, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


def line(text, fragment):
    found = [row for row in text.splitlines() if fragment in row]
    assert len(found) == 1, (fragment, len(found))
    return found[0]


def build():
    read, run = c.read, c.run
    lock = json.loads(read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        repo = ROOT / 'reference' / part
        assert run(['git', '-C', repo, 'rev-parse', 'HEAD']).stdout.decode().strip() == lock[part]['commit']
        run(['git', '-C', repo, 'diff', '--quiet', 'HEAD', '--'])
    classes = RUN / 'classes'
    classes.mkdir(parents=True, exist_ok=True)
    for stale in classes.rglob('*.class'):
        stale.unlink()
    jars = c.artifacts()
    std = next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    artifact = json.loads(read(ROOT / 'oracle/auth/json.lock.json'))
    simple = RUN / 'json-simple-1.1.1.jar'
    if not simple.exists() or hashlib.sha256(simple.read_bytes()).hexdigest() != artifact['sha256']:
        with urllib.request.urlopen(artifact['url'], timeout=60) as response:
            data = response.read()
        assert len(data) == artifact['size'] and hashlib.sha256(data).hexdigest() == artifact['sha256']
        simple.write_bytes(data)
    consts, consts_hash = constants_jar(read, run, lock)

    def source(name):
        return read(SOURCE / name)

    skills = source('core/game/node/entity/skill/Skills.java')
    weapon = source(COMBAT + 'equipment/WeaponInterface.java')
    pouch = source('content/global/skill/summoning/SummoningPouch.java')
    assert 'private SummoningPouch(int slot, int pouchId,' in pouch
    pouches = []
    for name in ['WOLPERTINGER_POUCH', 'GEYSER_TITAN_POUCH', 'IRON_TITAN_POUCH', 'STEEL_TITAN_POUCH']:
        pouches.append(name + '(' + re.search(r'\b' + name + r'\(\d+, (?:(?:true|false), )?(\d+),', pouch).group(1) + ')')
    graphics = source('core/game/world/update/flag/context/Graphics.java')
    body = graphics[graphics.index('{', graphics.index('public class Graphics')) + 1:graphics.rindex('}')]
    body = body.replace(c.method(graphics, 'public static void send(Graphics graphic, Location l)'), '')
    config = source('core/game/system/config/ItemConfigParser.kt')
    pathfinder = source(WORLD + 'path/Pathfinder.java')
    prayer = source('core/game/node/entity/player/link/prayer/PrayerType.java')
    spell = source(COMBAT + 'spell/SpellType.java')
    node = source('core/game/node/Node.java')
    entity = source('core/game/node/entity/Entity.java')
    closest = c.method(entity, 'public Location getClosestOccupiedTile(@NotNull Location other)')
    signature = 'public Location getClosestOccupiedTile(@NotNull Location other)'
    api = source('core/api/ContentAPI.kt')
    macro = source('core/game/system/timer/impl/AntiMacro.kt')
    values = dict(
        SKILL_CONSTANTS='\n'.join(re.findall(r'^\s*public static final int (?:ATTACK|NUM_SKILLS) = .*;$', skills, re.M)),
        POUCH_CONSTANTS=','.join(pouches),
        SLOT_CONSTANTS=re.search(r'public static final int SLOT_HAT = .*;', source('core/game/container/impl/EquipmentContainer.java')).group(),
        ITEM_CONFIG_CONSTANTS='\n'.join(f' public static final String {n} = "{v}";' for n, v in re.findall(r'const val (\w+) = "(.*)"', config)),
        PRAYER_CONSTANTS=','.join(re.findall(r'^\t([A-Z_]+)\(', prayer, re.M)),
        SPELL_TYPE_CONSTANTS=','.join(re.findall(r'^\t([A-Z_]+)\(', spell, re.M)),
        WEAPON_INTERFACE_CONSTANTS='\n'.join(re.findall(r'^\s*public static final int (?:BONUS|STYLE)_\w+ = \d+;$', weapon, re.M)),
        WEAPON_INTERFACE_TYPES=c.method(weapon, 'public static class AttackStyle') + '\n' + c.method(weapon, 'public static enum WeaponInterfaces'),
        GRAPHICS_BODY=body,
        # Verbatim Node.getCenterLocation, and Entity.getClosestOccupiedTile under a private name
        # with its occupied-tile helper; the stand-in records the call and then runs it.
        NODE_CENTER=c.method(node, 'public Location getCenterLocation()'),
        ENTITY_CLOSEST=closest.replace(signature, 'Location closestOccupiedTile(Location other)') + '\n'
        + c.method(entity, 'public List<Location> getOccupiedTiles()'),
        EVENT_NPC=line(macro, 'const val EVENT_NPC = ').strip(),
        GET_ATTRIBUTE=c.method(api, 'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T'),
    )
    assert signature in closest
    java, kotlin = [], []

    def adapter(name, content):
        path = RUN / 'adapters' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding='utf-8', newline='\n')
        (java if path.suffix == '.java' else kotlin).append(path)

    stale = RUN / 'adapters'
    if stale.is_dir():
        for path in sorted(stale.rglob('*'), reverse=True):
            path.unlink() if path.is_file() else path.rmdir()
    files = split(read(ROOT / 'oracle/combat-swing/adapters.txt'))
    for name in DROPPED:
        del files[name]
    files.update(split(read(HERE / 'adapters.txt')))
    for name, content in files.items():
        for key, value in values.items():
            content = content.replace('/*' + key + '*/', value)
        assert not re.search(r'/\*[A-Z_]+\*/', content), name
        adapter(name, content)
    location = source(WORLD + 'Location.kt')
    within = c.method(location, '@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)')
    adapter(WORLD + 'Location.kt', '''package core.game.world.map
import kotlin.math.min
import kotlin.math.sqrt
// The original packed coordinates, constructor normalisation and the members combat reads, verbatim.
class Location(x: Int, y: Int, z: Int) {
    var hash = LocationHash(x, y, (z % 4 + 4) % 4)
    val x: Int get() = hash.x
    val y: Int get() = hash.y
    val z: Int get() = hash.z
    companion object {
        @JvmStatic fun create(x: Int, y: Int, z: Int): Location = Location(x, y, z)
''' + c.method(location, '@JvmStatic fun getDistance(first: Location, second: Location)') + '''
    }
''' + '\n'.join(line(location, text) for text in [
        'override fun hashCode(): Int = hash.hash', 'val regionId: Int get()',
        'fun transform(diffX: Int, diffY: Int, z: Int)', '@JvmOverloads fun transform(dir: Direction, steps: Int = 1)',
        'fun getDistance(other: Location)']) + '\n' + c.method(location, 'override fun equals(other: Any?): Boolean') + '\n'
            + within.replace('@JvmOverloads fun withinDistance(other: Location, dist: Int = MapDistance.RENDERING.distance)',
                             'fun withinDistance(other: Location, dist: Int)') + '\n'
            + c.method(location, 'fun getStepComponents(dir: Direction)') + '\n'
            + c.method(location, 'fun deriveDirection(location: Location)') + '\n@JvmInline '
            + c.method(location, 'value class LocationHash(') + '\n}\n')
    adapter(WORLD + 'path/Pathfinder.java', 'package core.game.world.map.path;\nimport core.game.world.map.Location;\n'
            '// The PREVENT constants, the projectile singleton and isStandingIn are the original text.\npublic abstract class Pathfinder{\n'
            + '\n'.join(re.findall(r'^\s*public static final int PREVENT_\w+ = \w+;$', pathfinder, re.M)) + '\n'
            + line(pathfinder, 'public static final ProjectilePathfinder PROJECTILE') + '\n'
            + 'public abstract Path find(Location a,int s,Location b,int sx,int sy,int r,int t,int f,boolean n,ClipMaskSupplier c);\n'
            + c.method(pathfinder, 'public static boolean isStandingIn(') + '\n}\n')
    randoms = source('core/tools/RandomFunction.java')
    chosen = ['public static final int random(int a, int b)', 'public static boolean roll(int chance)',
              'public static final int random(int maxValue)', 'public static int randomize(int value)']
    adapter('core/tools/RandomFunction.java', 'package core.tools;\n// Selected original methods, verbatim; RANDOM is assignable so a case can bind its draws.\npublic class RandomFunction{public static java.util.Random RANDOM;\n'
            + '\n'.join(c.method(randoms, s) for s in chosen) + '}\n')
    for name in UNMODIFIED:
        path = SOURCE / name
        read(path)
        (java if path.suffix == '.java' else kotlin).append(path)
    read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    deps = os.pathsep.join(map(str, [std, simple, consts]))
    cp = os.pathsep.join(map(str, [classes, std, simple, consts]))
    run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
         '-jvm-target', '11', '-nowarn', '-classpath', deps, '-d', classes, *kotlin, *java])
    run(['javac', '--release', '11', '-nowarn', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    return lock, cp, consts_hash


def execute(cp, cases):
    exe = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    process = subprocess.run([str(exe), '-Xmx1g', '-Dfile.encoding=UTF-8', '-cp', cp, 'oracle.Support'], input=payload,
                             capture_output=True, cwd=ROOT, timeout=900)
    if process.returncode:
        raise RuntimeError(process.stderr.decode(errors='replace'))
    results = [json.loads(row) for row in process.stdout.splitlines() if row]
    assert len(results) == len(cases), (len(results), len(cases))
    return results


def generate(check):
    lock, cp, consts_hash = build()
    planned = corpus.cases()
    cases = [case for case, _ in planned]
    results = execute(cp, cases)
    # Second pass: every service call and draw of the marked operations fails once.
    failing = []
    for (case, sweep), result in zip(planned, results):
        if sweep is None:
            continue
        calls = sum(1 for event in result[sweep]['events'] if event[0] not in corpus.MARKERS)
        for at in range(1, calls + 1):
            ops = [dict(op, failAt=at) if k == sweep else op for k, op in enumerate(case['ops'][:sweep + 1])]
            failing.append(dict(case, ops=ops))
    cases += failing
    results += execute(cp, failing)
    # Any other failure class is a corpus or stand-in mistake, never an original outcome to record.
    mistakes = {}
    for number, (case, result) in enumerate(zip(cases, results)):
        for op, row in zip(case['ops'], result):
            error = row['error']
            if error not in (None, 'Null', 'Index', 'Binding') or (error == 'Binding' and 'failAt' not in op):
                mistakes.setdefault(error, (number, op))
    assert not mistakes, mistakes
    data = ''.join(json.dumps(dict(input=case, expected=result), sort_keys=True, separators=(',', ':')) + '\n'
                   for case, result in zip(cases, results)).encode()
    c.read(Path(__file__))
    c.read(HERE / 'corpus.py')
    c.read(ROOT / 'oracle/login/generate.py')
    operations = sum(len(case['ops']) for case in cases)
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(cases), operationCount=operations,
        failureCaseCount=len(failing), fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm='sha256, CR removed',
        kotlinCompiler='1.8.20', constantsJar=dict(source='reference/server git ' + lock['server']['commit'] + ':Server/libs/ConstLib-1.4.jar',
                                                   sha256=consts_hash, missingNamedConstant='Sounds.HUMAN_ATTACK_2564',
                                                   numericAdapter={'HUMAN_ATTACK_2564': 2564}),
        unmodified=UNMODIFIED,
        scope='Unmodified CombatPulse, ImpactHandler, Pulse, CombatSwingHandler, MeleeSwingHandler, RangeSwingHandler, MagicSwingHandler, '
              'BattleState, CombatStyle, InteractionType, Weapon, RangeWeapon, Ammunition, ArmourSet, BoltEffect, EquipmentSlot, Direction, '
              'Point, Path, ProjectilePathfinder, ClipMaskSupplier and ZoneType. Verbatim selected text: RandomFunction methods, Location '
              'packing/distance/transform/withinDistance/deriveDirection/getStepComponents/regionId/equals, the Pathfinder PREVENT constants '
              'and isStandingIn, Node.getCenterLocation, Entity.getClosestOccupiedTile/getOccupiedTiles/attack, ContentAPI getAttribute, '
              'AntiMacro.EVENT_NPC, Graphics constructors and the AttackStyle/WeaponInterfaces declarations. Every other class is a '
              'recording stand-in answering from the case input or from a small modelled world: lifepoints, prayer points, recoil damage, '
              'equipment replacement, attributes, death after startDeath, an optional retaliating Entity.onImpact, a PulseManager that '
              'knows only the combat pulse, and a one-queue per-tick runner. SalamanderSwingHandler is a class-identity stand-in. '
              'System.currentTimeMillis is not bound: only the +10000 offset of combat-time is recorded. Entity.hashCode is a case input. '
              'Does not drive BoltEffect overrides other than the DIAMOND path, removeFor, spell or special-attack content, DeathTask, '
              'or any original class behind a stand-in.')
    folder = ROOT / 'fixtures/combat-pulse'
    if check:
        assert (folder / 'oracle.jsonl').read_bytes() == data, 'combat-pulse fixture differs'
        assert json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) == provenance, 'combat-pulse provenance differs'
        print(f'{len(cases)} original combat pulse cases ({operations} operations) reproduced.')
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'oracle.jsonl').write_bytes(data)
        (folder / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {len(cases)} original combat pulse cases ({operations} operations).')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
