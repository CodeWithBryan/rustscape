"""Reproduce what the original dragonfire and switching NPC attacks do.

DragonfireSwingHandler, FireType, SwitchAttack, MultiSwingHandler.kt,
MixedSwingHandlers.kt, DragonfireShieldListener.kt (with DFSItemPlugin),
NodeTask, TickUtils.kt, IntType.kt, Container.kt, Plugin, Initializable and
TimerFlag are compiled unmodified on JDK 11 together with the unmodified
combat slice the combat-pulse oracle compiles (CombatPulse, ImpactHandler,
Pulse, the four swing handlers, BattleState and the equipment data classes).
Everything they reach outside that slice is a recording stand-in:
oracle/combat-swing/adapters.txt, then oracle/combat-pulse/adapters.txt,
oracle/special-attacks/adapters.txt and this directory's adapters.txt
replacing or adding files. Selected original text is inserted at generation
time. Java/Kotlin are test oracles only; --check never rewrites fixtures.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
RUN = ROOT / 'runs/dragonfire-oracle'
SOURCE = ROOT / 'reference/server/Server/src/main'
COMBAT = 'core/game/node/entity/combat/'
WORLD = 'core/game/world/map/'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
corpus = load('dragonfire_corpus', HERE / 'corpus.py')

COMBAT_SLICE = [
    COMBAT + 'CombatSwingHandler.kt', COMBAT + 'MeleeSwingHandler.kt', COMBAT + 'RangeSwingHandler.kt',
    COMBAT + 'MagicSwingHandler.kt', COMBAT + 'BattleState.java', COMBAT + 'CombatStyle.java',
    COMBAT + 'InteractionType.java', COMBAT + 'CombatPulse.kt', COMBAT + 'ImpactHandler.java',
    COMBAT + 'equipment/Weapon.java', COMBAT + 'equipment/RangeWeapon.java',
    COMBAT + 'equipment/Ammunition.java', COMBAT + 'equipment/ArmourSet.java', COMBAT + 'equipment/BoltEffect.java',
    'core/api/EquipmentSlot.kt', 'core/game/system/task/Pulse.java', WORLD + 'Direction.java', WORLD + 'Point.java',
    WORLD + 'path/Path.java', WORLD + 'path/ProjectilePathfinder.java', WORLD + 'path/ClipMaskSupplier.java',
    WORLD + 'zone/ZoneType.java',
]
PORTED = [
    'content/global/handlers/item/equipment/special/DragonfireSwingHandler.java', COMBAT + 'equipment/FireType.java',
    COMBAT + 'equipment/SwitchAttack.java', COMBAT + 'MultiSwingHandler.kt', COMBAT + 'MixedSwingHandlers.kt',
    'content/global/handlers/item/DragonfireShieldListener.kt', 'content/global/handlers/npc/ChromaticDragonNPC.java',
    'content/global/handlers/npc/MetalDragonNPC.java', 'content/global/handlers/npc/ChromaticDragonBehavior.kt',
]
UNMODIFIED = PORTED + ['core/game/system/task/NodeTask.java', 'core/tools/TickUtils.kt', 'core/game/interaction/IntType.kt',
                       'core/api/Container.kt', 'core/plugin/Plugin.java', 'core/plugin/Initializable.java',
                       'core/game/system/timer/TimerFlag.kt'] + COMBAT_SLICE
# Stand-ins of the swing corpus that an unmodified class or a generated file replaces (as in the combat-pulse oracle).
DROPPED = [
    'core/game/world/GameWorld.java', 'core/game/system/task/Pulse.java', WORLD + 'Direction.java',
    WORLD + 'path/ClipMaskSupplier.java', WORLD + 'path/Path.java', WORLD + 'path/Pathfinder.java',
    COMBAT + 'ImpactHandler.java',
]
# Stand-ins of the earlier corpora that this one replaces with another file type, or with the published constants.
REPLACED = ['core/game/system/timer/RSTimer.java', 'org/rs09/consts/Sounds.java']
CONTENT_API = [
    'fun registerTimer (entity: Entity, timer: RSTimer?) {', 'fun spawnTimer (identifier: String, vararg args: Any) : RSTimer? {',
    'fun removeAttribute(entity: Entity, attribute: String) {', 'fun <A, G> visualize(entity: Entity, anim: A, gfx: G) {',
    'fun hasDragonfireShieldProtection(player: Player, wyvern: Boolean = false): Boolean {',
    'fun calculateDragonfireMaxHit(entity: Entity, maxDamage: Int, wyvern: Boolean = false, unprotectableDamage: Int = 0, sendMessage: Boolean = false): Int {',
]
SETTINGS = ['public void toggleSpecialBar()', 'public void setSpecialToggled(boolean enable)', 'public boolean isSpecialToggled()',
            'public boolean drainSpecial(int amount)', 'public void setSpecialEnergy(int value)', 'public int getSpecialEnergy()']
SCOPE = (
    'DragonfireSwingHandler, FireType, SwitchAttack, MultiSwingHandler.kt, MixedSwingHandlers.kt and DragonfireShieldListener.kt '
    '(with DFSItemPlugin), ChromaticDragonNPC, MetalDragonNPC and ChromaticDragonBehavior.kt unmodified, with NodeTask, TickUtils.kt, IntType.kt, Container.kt, Plugin, Initializable and TimerFlag and the '
    'unmodified combat slice of the combat-pulse oracle (CombatPulse, ImpactHandler, Pulse, CombatSwingHandler, MeleeSwingHandler, '
    'RangeSwingHandler, MagicSwingHandler, BattleState, CombatStyle, InteractionType, Weapon, RangeWeapon, Ammunition, ArmourSet, '
    'BoltEffect, EquipmentSlot, Direction, Point, Path, ProjectilePathfinder, ClipMaskSupplier, ZoneType) on JDK 11 with Kotlin 1.8.20, '
    'JSON-simple 1.1.1 and the published ConstLib 1.4.28 (every Sounds and Items name checked against its suffix). Verbatim selected '
    'text: ContentAPI calculateDragonfireMaxHit, hasDragonfireShieldProtection, registerTimer, spawnTimer, removeAttribute, visualize and '
    'getAttribute, Player.getDragonfireProtection and getSwingHandler, Entity.isPlayer, Node.asItem and getCenterLocation, the special '
    'energy members of Settings, RandomFunction methods, the Location members combat reads, the Pathfinder constants and isStandingIn, '
    'Entity.getClosestOccupiedTile, AntiMacro.EVENT_NPC, the Graphics constructors and the AttackStyle/WeaponInterfaces declarations. '
    'Every other class is a recording stand-in answering from the case input or from a small modelled world (lifepoints, prayer points, '
    'attributes, equipment and inventory items with their packed id and charge, death, a pulse manager that knows only the combat pulse, '
    'a one-queue runner). TimerManager scheduling, the projectile class, setVarp, delayEntity, replaceSlot, DropListener, ItemPlugin '
    'registration, InteractionListeners, the AbstractNPC constructors, NPCBehavior and the item definitions are stand-ins; System.currentTimeMillis is not bound.'
)


def split(text):
    parts = re.split(r'^//// (\S+)\n', text, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


def line(text, fragment):
    found = [row for row in text.splitlines() if fragment in row]
    assert len(found) == 1, (fragment, len(found))
    return found[0]


def pinned(name, lock_path):
    """A pinned jar from this run directory or another oracle's; this generator never downloads."""
    pin = json.loads(c.read(ROOT / lock_path))['sha256']
    candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
    jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
    if jar is None:
        raise RuntimeError(f'{name} matching its pin was not found in runs/dragonfire-oracle or another runs/*-oracle directory')
    if jar != RUN / name:
        shutil.copyfile(jar, RUN / name)
    return RUN / name


def constants_match_names(classpath, texts):
    """Every Sounds/Items constant named carries its value as its suffix; check the jar agrees."""
    javap = c.JAVA_HOME / 'bin' / ('javap.exe' if os.name == 'nt' else 'javap') if c.JAVA_HOME else 'javap'
    sources = ''.join(texts)
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
    read, run = c.read, c.run
    lock = json.loads(read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        repo = ROOT / 'reference' / part
        assert run(['git', '-C', repo, 'rev-parse', 'HEAD']).stdout.decode().strip() == lock[part]['commit']
        run(['git', '-C', repo, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    classes = RUN / 'classes'
    if classes.exists():
        shutil.rmtree(classes)
    classes.mkdir()
    jars = c.artifacts()
    std = next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    simple = pinned('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json')
    consts = pinned('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')

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
    player = source('core/game/node/entity/player/Player.java')
    settings = source('core/game/node/entity/player/link/Settings.java')
    swing_signature = 'public CombatSwingHandler getSwingHandler(boolean swing)'
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
        NODE_CENTER=c.method(node, 'public Location getCenterLocation()'),
        NODE_AS_ITEM=c.method(node, 'public Item asItem()'),
        ENTITY_IS_PLAYER=c.method(entity, 'public boolean isPlayer()'),
        ENTITY_CLOSEST=closest.replace(signature, 'Location closestOccupiedTile(Location other)') + '\n'
        + c.method(entity, 'public List<Location> getOccupiedTiles()'),
        EVENT_NPC=line(macro, 'const val EVENT_NPC = ').strip(),
        GET_ATTRIBUTE=c.method(api, 'fun <T> getAttribute(entity: Entity, attribute: String, default: T): T'),
        # Player.getSwingHandler under a private name; the stand-in records the call and then runs it.
        PLAYER_SWING_HANDLER=c.method(player, swing_signature).replace(swing_signature, 'CombatSwingHandler originalSwingHandler(boolean swing)'),
        PLAYER_DRAGONFIRE=c.method(player, 'public int getDragonfireProtection(boolean fire)'),
        SETTINGS_FIELDS=line(settings, 'private boolean specialToggled;').strip() + '\n ' + line(settings, 'private int specialEnergy = 100;').strip(),
        SETTINGS_MEMBERS='\n '.join(c.method(settings, s) for s in SETTINGS),
        API_ORIGINAL='\n'.join(c.method(api, s) for s in CONTENT_API),
    )
    assert signature in closest
    java, kotlin = [], []
    texts = []

    def adapter(name, content):
        path = RUN / 'adapters' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding='utf-8', newline='\n')
        texts.append(content)
        (java if path.suffix == '.java' else kotlin).append(path)

    stale = RUN / 'adapters'
    if stale.is_dir():
        shutil.rmtree(stale)
    files = split(read(ROOT / 'oracle/combat-swing/adapters.txt'))
    for name in DROPPED:
        del files[name]
    files.update(split(read(ROOT / 'oracle/combat-pulse/adapters.txt')))
    for name in REPLACED:
        del files[name]
    files.update(split(read(ROOT / 'oracle/special-attacks/adapters.txt')))
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
            '// The PREVENT constants, the projectile singleton and isStandingIn are the original text. find(mover, destination, near,\n'
            '// finder) records the call and answers a path whose success is the case\'s ("dumbPath").\npublic abstract class Pathfinder{\n'
            + '\n'.join(re.findall(r'^\s*public static final int PREVENT_\w+ = \w+;$', pathfinder, re.M)) + '\n'
            + line(pathfinder, 'public static final ProjectilePathfinder PROJECTILE') + '\n'
            + 'public static final Pathfinder DUMB=new Pathfinder(){public Path find(Location a,int s,Location b,int sx,int sy,int r,int t,int f,boolean n,ClipMaskSupplier c){throw oracle.Support.unsupported("DumbPathfinder.find");}};\n'
            + 'public abstract Path find(Location a,int s,Location b,int sx,int sy,int r,int t,int f,boolean n,ClipMaskSupplier c);\n'
            + 'public static Path find(core.game.node.entity.Entity mover,Location destination,boolean near,Pathfinder finder){oracle.Support.emit("dumbPath",mover.tag,oracle.Support.coord(destination),near,finder==DUMB);Path p=new Path();p.setSuccesful(!Boolean.FALSE.equals(mover.spec.get("dumbPath")));return p;}\n'
            + c.method(pathfinder, 'public static boolean isStandingIn(') + '\n}\n')
    randoms = source('core/tools/RandomFunction.java')
    chosen = ['public static final int random(int a, int b)', 'public static boolean roll(int chance)',
              'public static final int random(int maxValue)', 'public static int randomize(int value)']
    adapter('core/tools/RandomFunction.java', 'package core.tools;\n// Selected original methods, verbatim; RANDOM is assignable so a case can bind its draws.\npublic class RandomFunction{public static java.util.Random RANDOM;\n'
            + '\n'.join(c.method(randoms, s) for s in chosen) + '}\n')
    for name in UNMODIFIED:
        path = SOURCE / name
        texts.append(read(path))
        (java if path.suffix == '.java' else kotlin).append(path)
    texts.append(read(HERE / 'Support.java'))
    java.append(HERE / 'Support.java')
    deps = os.pathsep.join(map(str, [std, simple, consts]))
    cp = os.pathsep.join(map(str, [classes, std, simple, consts]))
    run(['java', '-Xmx1g', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect',
         '-jvm-target', '11', '-nowarn', '-classpath', deps, '-d', classes, *kotlin, *java])
    run(['javac', '--release', '11', '-nowarn', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    checked = constants_match_names(deps, texts)
    return lock, cp, checked


def execute(cp, cases):
    exe = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    payload = ''.join(json.dumps(case) + '\n' for case in cases).encode()
    process = subprocess.run([str(exe), '-Xmx1g', '-Dfile.encoding=UTF-8', '-Duser.language=en', '-Duser.country=US', '-cp', cp, 'oracle.Support'],
                             input=payload, capture_output=True, cwd=ROOT, timeout=1800)
    if process.returncode:
        raise RuntimeError(process.stderr.decode(errors='replace'))
    results = [json.loads(row) for row in process.stdout.splitlines() if row]
    assert len(results) == len(cases), (len(results), len(cases))
    return results


def generate(check):
    lock, cp, checked = build()
    planned = corpus.cases()
    cases = [case for case, _ in planned]
    results = execute(cp, cases)
    # Second pass: every service call and draw of the marked operation fails once in turn.
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
    c.read(HERE / 'adapters.txt')
    c.read(ROOT / 'oracle/login/generate.py')
    operations, events = {}, {}
    for case, result in zip(cases, results):
        for op, row in zip(case['ops'], result):
            operations[op['op']] = operations.get(op['op'], 0) + 1
            for event in row['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    for needed in corpus.NEEDED_EVENTS:
        if needed not in events:
            raise RuntimeError(f'the corpus never reaches {needed}')
    operation_count = sum(operations.values())
    provenance = dict(
        format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(cases), operationCount=operation_count,
        failureCaseCount=len(failing), fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm='sha256, CR removed',
        kotlinCompiler='1.8.20', constantsJar=dict(lock='oracle/bank/constants.lock.json', checkedAgainstNames=checked),
        coverage=dict(operations=dict(sorted(operations.items())), recordedCalls=sum(events.values()), events=dict(sorted(events.items())),
                      groups=corpus.groups(cases)),
        unmodified=UNMODIFIED, ported=PORTED, scope=SCOPE)
    folder = ROOT / 'fixtures/dragonfire'
    summary = f'{len(cases)} original dragonfire and switching attack cases ({operation_count} operations, {len(failing)} failure cases)'
    if check:
        assert (folder / 'oracle.jsonl').read_bytes() == data, 'dragonfire fixture differs'
        assert json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) == provenance, 'dragonfire provenance differs'
        print(f'{summary} reproduced.')
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'oracle.jsonl').write_bytes(data)
        (folder / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {summary}.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
