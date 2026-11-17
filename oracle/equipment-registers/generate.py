"""Reproduce what the original equipment registers, the chinchompa and
salamander swing handlers and the remaining global creature classes do.

BarrowsEquipment.kt, BarrowsEquipmentRegister.kt, CrystalEquipmentRegister.kt,
EquipmentDegrader.kt, HastaEquipListener.kt, PVPEquipmentRegister.kt,
ChinchompaSwingHandler.java, SalamanderSwingHandler.kt, SheepBehavior.kt,
ChinchompaDeathListener.kt, ShooAwayStrayDogListener.kt, CowMilkingPlugin.java
and DecantListener.kt are compiled unmodified on JDK 11 with ApiExtensions.kt,
MultihitTargets.kt, ContentInterface.kt, StartupListener.kt, DialogueConst.kt
and the unmodified slice the dragonfire oracle compiles (its own classes, the
combat slice of the combat-pulse oracle, NodeTask, TickUtils.kt, IntType.kt,
Container.kt, Plugin, Initializable, TimerFlag). Everything they reach outside
that slice is a recording stand-in: oracle/combat-swing/adapters.txt, then
oracle/combat-pulse/adapters.txt, oracle/special-attacks/adapters.txt,
oracle/dragonfire/adapters.txt and this directory's adapters.txt replacing or
adding files. Selected original text is inserted at generation time.
Java/Kotlin are test oracles only; --check never rewrites fixtures.
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
RUN = ROOT / 'runs/equipment-registers-oracle'
SOURCE = ROOT / 'reference/server/Server/src/main'
COMBAT = 'core/game/node/entity/combat/'
WORLD = 'core/game/world/map/'
EQUIPMENT = 'content/global/handlers/item/equipment/'
NPC = 'content/global/handlers/npc/'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')
dragonfire = load('dragonfire_generate', ROOT / 'oracle/dragonfire/generate.py')
# One compiler module: its source hashes and JDK are shared with the dragonfire helpers this generator calls.
dragonfire.c = c
corpus = load('equipment_registers_corpus', HERE / 'corpus.py')

PORTED = [
    EQUIPMENT + 'BarrowsEquipment.kt', EQUIPMENT + 'BarrowsEquipmentRegister.kt', EQUIPMENT + 'CrystalEquipmentRegister.kt',
    EQUIPMENT + 'EquipmentDegrader.kt', EQUIPMENT + 'HastaEquipListener.kt', EQUIPMENT + 'PVPEquipmentRegister.kt',
    EQUIPMENT + 'special/ChinchompaSwingHandler.java', EQUIPMENT + 'special/SalamanderSwingHandler.kt',
    NPC + 'SheepBehavior.kt', NPC + 'ChinchompaDeathListener.kt', NPC + 'ShooAwayStrayDogListener.kt', NPC + 'CowMilkingPlugin.java',
    NPC + 'DecantListener.kt',
]
UNMODIFIED = PORTED + ['core/api/ApiExtensions.kt', COMBAT + 'MultihitTargets.kt', 'core/api/ContentInterface.kt',
                       'core/api/StartupListener.kt', 'core/tools/DialogueConst.kt'] + dragonfire.UNMODIFIED
# Stand-ins of the earlier corpora that the unmodified classes above replace.
REMOVED = [EQUIPMENT + 'special/ChinchompaSwingHandler.java', EQUIPMENT + 'special/SalamanderSwingHandler.kt']
CONTENT_API = dragonfire.CONTENT_API + [
    'fun impact(entity: Entity, amount: Int, type: ImpactHandler.HitsplatType = ImpactHandler.HitsplatType.NORMAL) {',
    'fun <T> setAttribute(entity: Entity, attribute: String, value: T) {',
    'fun inInventory(player: Player, id: Int, amount: Int = 1): Boolean {',
    'fun freeSlots(player: Player): Int {', 'fun lock(entity: Entity, duration: Int) {', 'fun unlock(entity: Entity) {',
    'fun stopWalk(entity: Entity) {',
]
SCOPE = (
    'BarrowsEquipment.kt, BarrowsEquipmentRegister.kt, CrystalEquipmentRegister.kt, EquipmentDegrader.kt, HastaEquipListener.kt, '
    'PVPEquipmentRegister.kt, ChinchompaSwingHandler.java, SalamanderSwingHandler.kt, SheepBehavior.kt, ChinchompaDeathListener.kt, '
    'ShooAwayStrayDogListener.kt, CowMilkingPlugin.java and DecantListener.kt unmodified, with ApiExtensions.kt, MultihitTargets.kt, '
    'ContentInterface.kt, StartupListener.kt, DialogueConst.kt and the unmodified slice of the dragonfire oracle (DragonfireSwingHandler, '
    'FireType, SwitchAttack, MultiSwingHandler.kt, MixedSwingHandlers.kt, DragonfireShieldListener.kt, ChromaticDragonNPC, '
    'MetalDragonNPC, ChromaticDragonBehavior.kt, NodeTask, TickUtils.kt, IntType.kt, Container.kt, Plugin, Initializable, TimerFlag '
    'and the combat slice of the combat-pulse oracle) on JDK 11 with Kotlin 1.8.20, JSON-simple 1.1.1 and the published ConstLib '
    '1.4.28 (every Sounds and Items name checked against its suffix). Verbatim selected text: the dragonfire oracle\'s, ContentAPI '
    'impact, setAttribute, inInventory, freeSlots, lock, unlock and stopWalk, Location.getSurroundingTiles, the Quests constant, '
    'SheepShearer\'s attribute, BarbarianTraining\'s attributes, Potion.getDose and the potion rows of Consumables (their ids). '
    'Every other class is a recording stand-in answering from the case input or from a small modelled world (lifepoints, '
    'attributes, equipment and inventory items with their packed id and charge, death, a pulse manager, a one-queue runner). '
    'CalquatDecant, decantContainer, the dialogue interpreter, InteractionListeners, scenery definitions, UseWithHandler, '
    'OptionHandler, ClassScanner, NPCBehavior, the item definitions and the other ContentAPI functions are stand-ins.'
)


def potion_rows(text):
    """The ids of every Potion and BarbarianMix row of Consumables, in declaration order."""
    rows = re.findall(r'^\s*([A-Z0-9_]+)\(new (Potion|BarbarianMix)\(new int\s*\[\]\s*\{([^}]*)\}', text, re.M)
    created = len(re.findall(r'new (?:Potion|BarbarianMix)\(', text))
    assert len(rows) == created, (len(rows), created)
    return '\n'.join(f' {name}(new int[]{{{ids.strip()}}}),' for name, _, ids in rows)


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
    dragonfire.RUN = RUN
    simple = dragonfire.pinned('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json')
    consts = dragonfire.pinned('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')

    def source(name):
        return read(SOURCE / name)

    line = dragonfire.line
    skills = source('core/game/node/entity/skill/Skills.java')
    weapon = source(COMBAT + 'equipment/WeaponInterface.java')
    pouch = source('content/global/skill/summoning/SummoningPouch.java')
    pouches = []
    for name in ['WOLPERTINGER_POUCH', 'GEYSER_TITAN_POUCH', 'IRON_TITAN_POUCH', 'STEEL_TITAN_POUCH']:
        pouches.append(name + '(' + re.search(r'\b' + name + r'\(\d+, (?:(?:true|false), )?(\d+),', pouch).group(1) + ')')
    graphics = source('core/game/world/update/flag/context/Graphics.java')
    body = graphics[graphics.index('{', graphics.index('public class Graphics')) + 1:graphics.rindex('}')]
    body = body.replace(c.method(graphics, 'public static void send(Graphics graphic, Location l)'), '')
    config = source('core/game/system/config/ItemConfigParser.kt')
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
    barbarian = source('content/region/kandarin/quest/barbariantraining/BarbarianTraining.kt')
    shearer = source('content/region/misthalin/lumbridge/quest/sheepshearer/SheepShearer.kt')
    quests = source('content/data/Quests.kt')
    potion = source('core/game/consumable/Potion.java')
    consumables = source('content/data/consumables/Consumables.java')
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
        PLAYER_SWING_HANDLER=c.method(player, swing_signature).replace(swing_signature, 'CombatSwingHandler originalSwingHandler(boolean swing)'),
        PLAYER_DRAGONFIRE=c.method(player, 'public int getDragonfireProtection(boolean fire)'),
        PLAYER_DEGRADER=line(player, 'public EquipmentDegrader degrader = new EquipmentDegrader();').strip(),
        SETTINGS_FIELDS=line(settings, 'private boolean specialToggled;').strip() + '\n ' + line(settings, 'private int specialEnergy = 100;').strip(),
        SETTINGS_MEMBERS='\n '.join(c.method(settings, s) for s in dragonfire.SETTINGS),
        API_ORIGINAL='\n'.join(c.method(api, s) for s in CONTENT_API),
        BARBARIAN_CONSTANTS='\n'.join(line(barbarian, f'const val {n} ').strip() for n in ['attributeHerblore', 'attributeHasta']),
        SHEEP_ATTRIBUTE=line(shearer, 'val ATTR_IS_PENGUIN_SHEEP_SHEARED = ').strip(),
        QUEST_CONSTANT=line(quests, 'SHEEP_SHEARER(').strip().rstrip(',') + ';',
        POTION_DOSE=c.method(potion, 'public int getDose(Item potion)'),
        POTION_ROWS=potion_rows(consumables),
        USE_WITH_TYPES='\n'.join(re.findall(r'^\s*public static final int \w+_TYPE = \d+;$', source('core/game/interaction/UseWithHandler.java'), re.M)),
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
    files = dragonfire.split(read(ROOT / 'oracle/combat-swing/adapters.txt'))
    for name in dragonfire.DROPPED:
        del files[name]
    files.update(dragonfire.split(read(ROOT / 'oracle/combat-pulse/adapters.txt')))
    for name in dragonfire.REPLACED:
        del files[name]
    files.update(dragonfire.split(read(ROOT / 'oracle/special-attacks/adapters.txt')))
    files.update(dragonfire.split(read(ROOT / 'oracle/dragonfire/adapters.txt')))
    for name in REMOVED:
        del files[name]
    files.update(dragonfire.split(read(HERE / 'adapters.txt')))
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
// The original packed coordinates, constructor normalisation and the members combat and the creatures read, verbatim.
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
            + c.method(location, 'val surroundingTiles: ArrayList<Location> get()') + '\n'
            + c.method(location, 'fun getStepComponents(dir: Direction)') + '\n'
            + c.method(location, 'fun deriveDirection(location: Location)') + '\n@JvmInline '
            + c.method(location, 'value class LocationHash(') + '\n}\n')
    pathfinder = source(WORLD + 'path/Pathfinder.java')
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
    checked = dragonfire.constants_match_names(deps, texts)
    return lock, cp, checked


def generate(check):
    lock, cp, checked = build()
    planned = corpus.cases()
    cases = [case for case, _ in planned]
    results = dragonfire.execute(cp, cases)
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
    results += dragonfire.execute(cp, failing)
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
    c.read(ROOT / 'oracle/dragonfire/generate.py')
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
    folder = ROOT / 'fixtures/equipment-registers'
    summary = (f'{len(cases)} original equipment register, chinchompa, salamander and creature cases '
               f'({operation_count} operations, {len(failing)} failure cases)')
    if check:
        assert (folder / 'oracle.jsonl').read_bytes() == data, 'equipment-registers fixture differs'
        assert json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) == provenance, 'equipment-registers provenance differs'
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
