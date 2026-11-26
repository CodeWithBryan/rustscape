"""Run the original AttackListener, Player debug members and GraveController.hasGraveAt on JDK 11.

Compiled unchanged: content/global/skill/AttackListener.kt (against the published ConstLib 1.4.28), IntType.kt,
Location.kt and Vector.kt. Original text inserted into adapters at generation time: Player.toggleDebug, Player.debug and
Player.isDebug (Player.java), GraveController.hasGraveAt (GraveController.kt). Everything else (InteractionListener's
registration, getSwingHandler, ContentAPI sendMessage, KillerwattBehavior.aggressUponKillerwatt, Entity.attack, the
attribute map, the packet dispatch, the rights) is a recording adapter answering from the case. Each case runs in a
class loader of its own (oracle/content-api/Boot.java). Java/Kotlin are test oracles only; --check never rewrites a
fixture.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import random
import shutil
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/helpers-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/helpers'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')

UNCHANGED = [
    'content/global/skill/AttackListener.kt',
    'core/game/interaction/IntType.kt',
    'core/game/world/map/Location.kt',
    'core/api/utils/Vector.kt',
]
SCOPE = (
    'AttackListener.kt, IntType.kt, Location.kt and Vector.kt compiled unchanged on JDK 11 with Kotlin 1.8.20 and the '
    'published ConstLib 1.4.28; Player.toggleDebug, Player.debug, Player.isDebug and GraveController.hasGraveAt inserted '
    'verbatim. InteractionListener.flagInstant/on, Player.getSwingHandler, ContentAPI sendMessage, '
    'KillerwattBehavior.aggressUponKillerwatt, Entity.attack, the attribute map, PacketDispatch.sendMessage and '
    'PlayerDetails.getRights are recording adapters. Every recorded call in order with its arguments, results, errors, '
    'the attribute map after each operation and injected failures of every recorded call. CombatPulse.attack is '
    'combat-pulse-oracle; ContentAPI sendMessage (splitLines) is content-api-oracle; the killerwatt behaviour is content.'
)

ADAPTERS = {
    'core/game/node/Node.java': r'''package core.game.node;
import core.game.world.map.Location;
public class Node {
    protected String name;
    protected Location location;
    protected int id;
    public core.game.interaction.DestinationFlag destinationFlag;
    public Node() {}
    public Node(int id) { this.id = id; }
    public Node(String name, Location location) { this.name = name; this.location = location; }
    public int getId() { return id; }
    public Location getLocation() { return location; }
}
''',
    'core/game/node/entity/Entity.java': r'''package core.game.node.entity;
public class Entity extends core.game.node.Node {
    public Entity() {}
    public Entity(int id) { super(id); }
    public void attack(core.game.node.Node node) { oracle.Support.ev("Entity.attack", this, node); }
}
''',
    'core/game/node/entity/npc/NPC.java': 'package core.game.node.entity.npc;\npublic class NPC extends core.game.node.entity.Entity { public NPC(int id) { super(id); } }\n',
    'core/game/node/entity/player/Player.java': r'''package core.game.node.entity.player;
import core.game.node.entity.combat.CombatSwingHandler;
import core.game.node.entity.player.info.Rights;
public class Player extends core.game.node.entity.Entity {
    private final PacketDispatch packetDispatch = new PacketDispatch();
    private final Details details = new Details();
    public PacketDispatch getPacketDispatch() { return packetDispatch; }
    public CombatSwingHandler getSwingHandler(boolean swing) { oracle.Support.ev("Player.getSwingHandler", swing); return oracle.Support.handler; }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        oracle.Support.ev("Entity.getAttribute", key);
        Object o = oracle.Support.attributes.get(key);
        return o != null ? (T) o : fail;
    }
    public void setAttribute(String key, Object value) { oracle.Support.ev("Entity.setAttribute", key, value); oracle.Support.attributes.put(key, value); }
    /* ORIGINAL_TOGGLE_DEBUG */
    /* ORIGINAL_DEBUG */
    /* ORIGINAL_IS_DEBUG */
    public static class PacketDispatch { public void sendMessage(String s) { oracle.Support.ev("PacketDispatch.sendMessage", s); } }
    public static class Details { public Rights getRights() { oracle.Support.ev("PlayerDetails.getRights"); return oracle.Support.rights; } }
}
''',
    'core/game/node/entity/player/info/Rights.java': 'package core.game.node.entity.player.info;\npublic enum Rights { REGULAR_PLAYER, PLAYER_MODERATOR, ADMINISTRATOR }\n',
    'core/game/node/entity/combat/CombatStyle.java': 'package core.game.node.entity.combat;\npublic enum CombatStyle { MELEE, RANGE, MAGIC }\n',
    'core/game/node/entity/combat/CombatSwingHandler.java': r'''package core.game.node.entity.combat;
public class CombatSwingHandler {
    private final CombatStyle type;
    public CombatSwingHandler(CombatStyle type) { this.type = type; }
    public CombatStyle getType() { return type; }
}
''',
    'core/game/node/entity/combat/graves/Grave.java': r'''package core.game.node.entity.combat.graves;
public class Grave extends core.game.node.entity.npc.NPC {
    public Grave(core.game.world.map.Location l) { super(6571); location = l; }
}
''',
    'core/game/node/entity/combat/graves/GraveController.kt': r'''package core.game.node.entity.combat.graves
import core.game.world.map.Location
class GraveController {
    companion object {
        val activeGraves = HashMap<Int, Grave>()
        /* ORIGINAL_HAS_GRAVE_AT */
    }
}
''',
    'core/game/interaction/InteractionListener.kt': r'''package core.game.interaction
import core.game.node.Node
import core.game.node.entity.player.Player
interface InteractionListener {
    fun defineListeners()
    fun on(type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) { oracle.Support.registered(type, option, handler) }
    fun flagInstant() { oracle.Support.ev("InteractionListener.flagInstant", this::class.java.name) }
}
''',
    'content/global/skill/slayer/monsters/KillerwattBehavior.kt': r'''package content.global.skill.slayer.monsters
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
class KillerwattBehavior {
    companion object {
        fun aggressUponKillerwatt(player: Player, self: NPC) { oracle.Support.ev("KillerwattBehavior.aggressUponKillerwatt", player, self) }
    }
}
''',
    'core/api/ContentAPI.kt': r'''package core.api
import core.game.node.entity.player.Player
fun sendMessage(player: Player, message: String) { oracle.Support.ev("ContentAPI.sendMessage", player, message) }
''',
    'core/game/interaction/DestinationFlag.java': 'package core.game.interaction;\npublic class DestinationFlag { public static final DestinationFlag LOCATION = new DestinationFlag(); }\n',
    'core/game/world/map/RegionManager.java': r'''package core.game.world.map;
public class RegionManager {
    public static Region forId(int id) { throw new UnsupportedOperationException(); }
    public static Location getTeleportLocation(Location l, int r) { throw new UnsupportedOperationException(); }
}
''',
    'core/game/world/map/Region.java': 'package core.game.world.map;\npublic class Region { public RegionChunk[][][] getChunks() { return null; } }\n',
    'core/game/world/map/RegionChunk.java': 'package core.game.world.map;\npublic class RegionChunk {}\n',
    'core/game/world/map/Direction.java': r'''package core.game.world.map;
public enum Direction {
    NORTH_WEST(-1, 1), NORTH(0, 1), NORTH_EAST(1, 1), WEST(-1, 0), EAST(1, 0), SOUTH_WEST(-1, -1), SOUTH(0, -1), SOUTH_EAST(1, -1);
    private final int stepX, stepY;
    Direction(int stepX, int stepY) { this.stepX = stepX; this.stepY = stepY; }
    public int getStepX() { return stepX; }
    public int getStepY() { return stepY; }
}
''',
    'core/game/world/map/MapDistance.java': 'package core.game.world.map;\npublic enum MapDistance { RENDERING; public int getDistance() { return 15; } }\n',
    'core/game/world/map/path/Path.java': 'package core.game.world.map.path;\npublic class Path { public boolean isSuccessful() { return false; } public java.util.LinkedList<core.game.world.map.Location> getPoints() { return new java.util.LinkedList<>(); } }\n',
    'core/game/world/map/path/Pathfinder.java': r'''package core.game.world.map.path;
public class Pathfinder {
    public static final Pathfinder DUMB = new Pathfinder();
    public static Path find(core.game.world.map.Location a, core.game.world.map.Location b, boolean near, Pathfinder f) { throw new UnsupportedOperationException(); }
}
''',
    'core/tools/RandomFunction.java': 'package core.tools;\npublic class RandomFunction { public static int getRandom(int max) { throw new UnsupportedOperationException(); } }\n',
}


def markers():
    player = c.read(SERVER / 'core/game/node/entity/player/Player.java')
    graves = c.read(SERVER / 'core/game/node/entity/combat/graves/GraveController.kt')
    return {
        'ORIGINAL_TOGGLE_DEBUG': c.method(player, 'public void toggleDebug() {'),
        'ORIGINAL_DEBUG': c.method(player, 'public void debug(String string) {'),
        'ORIGINAL_IS_DEBUG': c.method(player, 'public boolean isDebug() {'),
        'ORIGINAL_HAS_GRAVE_AT': c.method(graves, '@JvmStatic fun hasGraveAt(loc: Location) : Boolean {'),
    }


def libraries():
    found = []
    for name, lock_path in [('json-simple-1.1.1.jar', 'oracle/auth/json.lock.json'), ('ConstLib-1.4.28.jar', 'oracle/bank/constants.lock.json')]:
        pin = json.loads(c.read(ROOT / lock_path))['sha256']
        candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
        jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
        if jar is None:
            raise RuntimeError(f'{name} matching its pin was not found in runs/*-oracle (this generator never downloads)')
        if jar != RUN / name:
            shutil.copyfile(jar, RUN / name)
        found.append(RUN / name)
    return found


def build():
    lock = json.loads(c.read(ROOT / 'upstream.lock.json'))
    reference = ROOT / 'reference' / 'server'
    if c.run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock['server']['commit']:
        raise RuntimeError('Reference pin differs')
    c.run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    stdlib = next(path for path in jars if path.name.startswith('kotlin-stdlib-'))
    annotations = next(path for path in jars if path.name.startswith('annotations-'))
    libs = [stdlib, annotations, *libraries()]
    kotlin, java = [], []
    for name in UNCHANGED:
        path = SERVER / name
        c.read(path)
        (kotlin if path.suffix == '.kt' else java).append(path)
    inserted = markers()
    folder = RUN / 'adapters'
    if folder.exists():
        shutil.rmtree(folder)
    for name, text in sorted(ADAPTERS.items()):
        for marker, original in inserted.items():
            text = text.replace(f'/* {marker} */', original)
        if 'ORIGINAL_' in text:
            raise RuntimeError(f'Unfilled marker in {name}')
        target = folder / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding='utf-8', newline='\n')
        (kotlin if target.suffix == '.kt' else java).append(target)
    c.read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    boot_source = ROOT / 'oracle/content-api/Boot.java'
    c.read(boot_source)
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
    c.run(['javac', '--release', '11', '-encoding', 'UTF-8', '-nowarn', '-d', boot, boot_source])
    return lock, os.pathsep.join(map(str, [boot, *libs])), classes


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


# ------------------------------------------------------------------ cases
def case(name, ops, sweep=False, **extra):
    return dict(name=name, ops=ops, **extra), sweep


def cases():
    out = []
    styles = ['MELEE', 'RANGE', 'MAGIC', None, 'absent']
    # AttackListener: the registration, then the handler on every special id with every swing handler type.
    out.append(case('attack/define', [['define']], sweep=True, style='MELEE'))
    for npc_id in [1, 4474, 7891, 3201, 3202, 3200, 3203, 4475, 7890, 0, -1]:
        for style in styles:
            for npc in [True, False]:
                ops = [['define'], ['node', npc_id, npc], ['attack', 0]]
                out.append(case(f'attack/{npc_id}/{style}/{"npc" if npc else "node"}', ops, sweep=style in ('MELEE', 'MAGIC') and npc, style=style))
    out.append(case('attack/style-changes', [['define'], ['node', 4474, True], ['node', 7891, True], ['attack', 0], ['attack', 1], ['style', 'MAGIC'], ['attack', 0], ['attack', 1],
                                             ['style', None], ['attack', 0], ['attack', 1], ['style', 'absent'], ['attack', 0], ['attack', 1]], style='MELEE'))
    out.append(case('attack/registered-twice', [['define'], ['define'], ['node', 3202, True], ['attack', 0], ['attack', 0]], sweep=True, style='RANGE'))
    # Player.toggleDebug, debug and isDebug over every kind of stored attribute and every right.
    for stored in ['absent', True, False, 1, 0, 'true']:
        for rights in ['ADMINISTRATOR', 'PLAYER_MODERATOR', 'REGULAR_PLAYER', None]:
            attributes = {} if stored == 'absent' else {'debug': stored}
            ops = [['isDebug'], ['debug', 'first'], ['toggleDebug'], ['isDebug'], ['debug', 'second'], ['toggleDebug'], ['isDebug'], ['debug', ''], ['toggleDebug']]
            out.append(case(f'debug/{stored}/{rights}', ops, sweep=stored in ('absent', True) and rights in ('ADMINISTRATOR', 'REGULAR_PLAYER'), attributes=attributes, rights=rights))
    out.append(case('debug/attribute-changes', [['toggleDebug'], ['attribute', 'debug', 7], ['toggleDebug'], ['isDebug'], ['debug', 'x'], ['attribute', 'debug', None], ['toggleDebug'], ['rights', None], ['isDebug'],
                                                ['rights', 'ADMINISTRATOR'], ['isDebug'], ['attribute', 'debug', 'no'], ['isDebug'], ['debug', 'y'], ['attribute', 'other', True], ['isDebug']], rights='ADMINISTRATOR'))
    # GraveController.hasGraveAt: locations compare by their packed hash; a grave without a location matches nothing.
    home = [3222, 3218, 0]
    queries = [home, [3222, 3218, 1], [3222, 3218, 4], [3222, 3219, 0], [3222 + 32768, 3218, 0], [3222, 3218 - 32768, 0], [0, 0, 0], [-1, -1, 3], None]
    for graves in [[], [home], [None], [[0, 0, 0]], [None, home], [[3222, 3218, 1], [3222, 3219, 0]], [[3222, 3218, 4]], [[3222 - 32768, 3218 + 32768, 8]], [[-1, -1, 3], [16383, -16384, 2]]]:
        ops = [['grave', 100 + i, g] for i, g in enumerate(graves)] + [['hasGraveAt', q] for q in queries]
        out.append(case(f'graves/{graves}', ops))
    out.append(case('graves/removed', [['grave', 7, home], ['hasGraveAt', home], ['graveGone', 7], ['hasGraveAt', home], ['grave', 7, [3200, 3200, 0]], ['grave', 7, home], ['hasGraveAt', [3200, 3200, 0]], ['hasGraveAt', home]]))
    rng = random.Random(53532)
    for n in range(30):
        ops = []
        for i in range(rng.randrange(0, 12)):
            ops.append(['grave', rng.randrange(6), rng.choice([None, [rng.randrange(3220, 3224), rng.randrange(3216, 3220), rng.randrange(-4, 8)]])])
        for i in range(8):
            ops.append(['hasGraveAt', [rng.randrange(3220, 3224), rng.randrange(3216, 3220), rng.randrange(0, 4)]])
        out.append(case(f'graves/random/{n}', ops))
    return out


EXPECTED_ERRORS = (None, 'Null', 'Binding', 'Cast')


def generate(check):
    lock, classpath, classes = build()
    planned = cases()
    rows = execute(classpath, classes, [x for x, _ in planned])
    failing = []
    for (row_case, sweep), row in zip(planned, rows):
        if not sweep:
            continue
        for at in range(1, row['expected']['calls'] + 1):
            failing.append(dict(row_case, failAt=at, name=row_case['name'] + f'/fail{at}'))
    rows += execute(classpath, classes, failing)
    mistakes = {}
    for row in rows:
        for step in row['expected']['steps']:
            error = step.get('error')
            if error not in EXPECTED_ERRORS:
                mistakes.setdefault(error, row['name'])
            if error == 'Binding' and 'failAt' not in row:
                mistakes.setdefault(error, row['name'])
    if mistakes:
        raise RuntimeError(f'unexpected outcomes (corpus or stand-in mistakes): {mistakes}')
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':'), ensure_ascii=False) + '\n' for row in rows).encode('utf-8')
    c.read(pathlib.Path(__file__))
    c.read(ROOT / 'oracle/login/generate.py')
    operations, errors, events = {}, {}, {}
    steps = 0
    for row in rows:
        for op, step in zip(row['ops'], row['expected']['steps']):
            steps += 1
            operations[op[0]] = operations.get(op[0], 0) + 1
            if step.get('error'):
                errors[step['error']] = errors.get(step['error'], 0) + 1
            for event in step['events']:
                events[event[0]] = events.get(event[0], 0) + 1
    coverage = dict(cases=len(rows), operations=dict(sorted(operations.items())), steps=steps, recordedCalls=sum(events.values()),
                    events=dict(sorted(events.items())), errors=dict(sorted(errors.items())), failureCases=len(failing))
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows),
                      fixtureSha256=hashlib.sha256(data).hexdigest(), coverage=coverage, sourceHashAlgorithm='sha256, CR removed',
                      kotlinCompiler='1.8.20', unchanged=UNCHANGED, scope=SCOPE)
    summary = f'{len(rows)} original helper cases ({steps} operations, {coverage["recordedCalls"]} recorded calls, {len(failing)} failure cases)'
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('helpers fixtures/provenance differ; inspect before regeneration')
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
