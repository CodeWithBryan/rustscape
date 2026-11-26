"""Run the original runtime NPC members on JDK 11 and record what they do.

Original text inserted into adapters at generation time: the three NPC constructors, NPC.clear, NPC.transform,
NPC.transformWithHpCarryover and NPC.reTransform (NPC.java), Entity.init and Entity.clear (Entity.java). Compiled
unchanged: NodeList.java and NodeListIterator.java (Repository.getNpcs()), Location.kt, Vector.kt and EntityFlag.kt. Everything else
(NPCDefinition.forId, InteractPlugin, NPCBehavior, initConfig/configure, EntityFlags.getOrdinal, UpdateMasks, Skills,
TimerRegistry.addAutoTimers, the region chunk and tolerance removals, PulseManager.clear, the renderable list) is a
recording adapter answering from the case. Each case runs in a class loader of its own (oracle/content-api/Boot.java).
Java/Kotlin are test oracles only; --check never rewrites a fixture.
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
RUN = ROOT / 'runs/npc-runtime-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/npc-runtime'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')

UNCHANGED = [
    'core/game/world/repository/NodeList.java',
    'core/game/world/repository/NodeListIterator.java',
    'core/game/world/map/Location.kt',
    'core/api/utils/Vector.kt',
    'core/game/world/update/flag/EntityFlag.kt',
]
SCOPE = (
    'The NPC constructors, NPC.clear, NPC.transform, NPC.transformWithHpCarryover, NPC.reTransform, Entity.init and '
    'Entity.clear inserted verbatim into adapters, with NodeList.java, NodeListIterator.java, Location.kt, Vector.kt and EntityFlag.kt compiled '
    'unchanged on JDK 11 with Kotlin 1.8.20. NPCDefinition.forId, InteractPlugin, NPCBehavior, initConfig, configure, '
    'EntityFlags.getOrdinal, UpdateMasks, Skills lifepoints, TimerRegistry.addAutoTimers, RegionChunk.remove/removePlayer, '
    'the region tolerances, PulseManager.clear and the renderable NPC list are recording adapters. Every recorded call in '
    'order, every field after each operation, the repository slots, and injected failures of every recorded call. NPC.init '
    'and initConfig are npc-init-oracle; NodeList and the renderable list bodies are repository-oracle; RegionManager.move '
    'is entity-regions-oracle.'
)

ADAPTERS = {
    'core/game/node/Node.java': r'''package core.game.node;
import core.game.interaction.InteractPlugin;
import core.game.world.map.Direction;
import core.game.world.map.Location;
public class Node {
    protected String name;
    protected Location location;
    protected int index;
    protected Direction direction;
    protected int size = 1;
    protected boolean active = true;
    protected InteractPlugin interactPlugin;
    public core.game.interaction.DestinationFlag destinationFlag;
    public Node(String name, Location location) { this.name = name; this.location = location; }
    public core.game.node.entity.npc.NPC asNpc() { return (core.game.node.entity.npc.NPC) this; }
    public core.game.node.entity.player.Player asPlayer() { return (core.game.node.entity.player.Player) this; }
    public String getName() { return name; }
    public int getIndex() { return index; }
    public void setIndex(int index) { this.index = index; }
    public Location getLocation() { return location; }
    public int size() { return size; }
    public boolean isActive() { return active; }
    public void oracleLocation(Location l) { location = l; }
    public Direction oracleDirection() { return direction; }
    public Integer oraclePlugin() { return interactPlugin == null ? null : interactPlugin.serial; }
}
''',
    'core/game/node/entity/Entity.java': r'''package core.game.node.entity;
import core.game.node.Node;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.system.timer.TimerRegistry;
import core.game.world.map.Location;
public class Entity extends Node {
    public final PulseManager pulseManager = new PulseManager(this);
    private final Skills skills = new Skills(this);
    private final UpdateMasks updateMasks = new UpdateMasks(this);
    public Entity(String name, Location location) { super(name, location); oracle.Support.allocated(this); }
    public Skills getSkills() { return skills; }
    public UpdateMasks getUpdateMasks() { return updateMasks; }
    /* ORIGINAL_ENTITY_INIT */
    /* ORIGINAL_ENTITY_CLEAR */
    public static class PulseManager {
        final Entity e;
        PulseManager(Entity e) { this.e = e; }
        public void clear() { oracle.Support.ev("PulseManager.clear", e); }
    }
    public static class Skills {
        final Entity e;
        Skills(Entity e) { this.e = e; }
        public int getLifepoints() { oracle.Support.ev("Skills.getLifepoints", e); return oracle.Support.lifepoints.getOrDefault(e, 0); }
        public void setLifepoints(int lp) { oracle.Support.ev("Skills.setLifepoints", e, lp); oracle.Support.lifepoints.put(e, lp); }
    }
    public static class UpdateMasks {
        final Entity e;
        UpdateMasks(Entity e) { this.e = e; }
        public boolean unregisterSynced(int ordinal) { oracle.Support.ev("UpdateMasks.unregisterSynced", e, ordinal); return true; }
        public boolean register(core.game.world.update.flag.EntityFlag flag, Object context, boolean sync) { oracle.Support.ev("UpdateMasks.register", e, flag, context, sync); return true; }
    }
}
''',
    'core/game/node/entity/player/Player.java': r'''package core.game.node.entity.player;
public class Player extends core.game.node.entity.Entity {
    public Player(String name, core.game.world.map.Location location) { super(name, location); }
}
''',
    'core/game/node/entity/npc/NPC.java': r'''package core.game.node.entity.npc;
import core.cache.def.impl.NPCDefinition;
import core.game.interaction.InteractPlugin;
import core.game.node.entity.Entity;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import core.game.world.repository.Repository;
import core.game.world.update.flag.EFlagType;
import core.game.world.update.flag.EntityFlag;
import core.game.world.update.flag.EntityFlags;
public class NPC extends Entity {
    private int id;
    private final int originalId;
    private NPCDefinition definition;
    public NPCBehavior behavior;
    /* ORIGINAL_NPC_1 */
    /* ORIGINAL_NPC_3 */
    /* ORIGINAL_NPC_2 */
    /* ORIGINAL_NPC_CLEAR */
    /* ORIGINAL_NPC_TRANSFORM */
    /* ORIGINAL_NPC_TRANSFORM_HP */
    /* ORIGINAL_NPC_RETRANSFORM */
    public void initConfig() { oracle.Support.ev("NPC.initConfig", this); }
    public void configure() { oracle.Support.ev("NPC.configure", this); }
    public static NPC oracleNew(int id, Location l, Direction d) { return new NPC(id, l, d); }
    public int oracleId() { return id; }
    public int oracleOriginalId() { return originalId; }
    public NPCDefinition oracleDefinition() { return definition; }
}
''',
    'core/game/node/entity/npc/NPCBehavior.java': r'''package core.game.node.entity.npc;
public class NPCBehavior {
    public final int id;
    NPCBehavior(int id) { this.id = id; }
    public static NPCBehavior forId(int id) { oracle.Support.ev("NPCBehavior.forId", id); return new NPCBehavior(id); }
    public void onRemoval(NPC self) { oracle.Support.ev("NPCBehavior.onRemoval", id, self); }
}
''',
    'core/cache/def/impl/NPCDefinition.java': r'''package core.cache.def.impl;
public class NPCDefinition {
    public String label, name;
    public int size;
    public static NPCDefinition forId(int id) { return oracle.Support.definition(id); }
    public String getName() { return name; }
}
''',
    'core/game/interaction/InteractPlugin.java': r'''package core.game.interaction;
public class InteractPlugin {
    static int serials;
    public final int serial;
    final core.game.node.Node node;
    public InteractPlugin(core.game.node.Node node) { this.node = node; serial = serials++; oracle.Support.ev("InteractPlugin.<init>", node, serial); }
    public void setDefault() { oracle.Support.ev("InteractPlugin.setDefault", node, serial); }
}
''',
    'core/game/interaction/DestinationFlag.java': 'package core.game.interaction;\npublic class DestinationFlag { public static final DestinationFlag LOCATION = new DestinationFlag(), ENTITY = new DestinationFlag(); }\n',
    'core/game/system/timer/TimerRegistry.java': 'package core.game.system.timer;\npublic class TimerRegistry { public static void addAutoTimers(core.game.node.entity.Entity e) { oracle.Support.ev("TimerRegistry.addAutoTimers", e); } }\n',
    'core/game/world/update/flag/EFlagType.java': 'package core.game.world.update.flag;\npublic enum EFlagType { Player, NPC }\n',
    'core/game/world/update/flag/EntityFlags.java': r'''package core.game.world.update.flag;
public class EntityFlags {
    public static int getOrdinal(EFlagType type, EntityFlag flag) {
        if (type != EFlagType.NPC || flag != EntityFlag.TypeSwap) throw new AssertionError("unexpected flag lookup");
        return oracle.Support.ordinal();
    }
}
''',
    'core/game/world/repository/Repository.java': r'''package core.game.world.repository;
import core.game.node.entity.npc.NPC;
public class Repository {
    public static class Npcs extends NodeList<NPC> {
        Npcs(int capacity) { super(capacity); }
        @Override public boolean add(NPC n) { oracle.Support.ev("Repository.npcs.add", n); return super.add(n); }
        @Override public boolean remove(Object n) { oracle.Support.ev("Repository.npcs.remove", n); return super.remove(n); }
    }
    static Npcs npcs;
    public static void oracleCapacity(int capacity) { npcs = new Npcs(capacity); }
    public static Npcs getNpcs() { return npcs; }
    public static void addRenderableNPC(NPC n) { oracle.Support.ev("Repository.addRenderableNPC", n); if (!oracle.Support.renderable.contains(n)) oracle.Support.renderable.add(n); }
    public static void removeRenderableNPC(NPC n) { oracle.Support.ev("Repository.removeRenderableNPC", n); oracle.Support.renderable.remove(n); }
}
''',
    'core/game/world/map/RegionManager.java': r'''package core.game.world.map;
public class RegionManager {
    static final java.util.Map<Integer, Region> regions = new java.util.HashMap<>();
    public static Region forId(int id) { return regions.computeIfAbsent(id, Region::new); }
    public static Location getTeleportLocation(Location l, int r) { throw new UnsupportedOperationException(); }
}
''',
    'core/game/world/map/Region.java': r'''package core.game.world.map;
public class Region {
    final int id;
    final RegionChunk[][][] chunks = new RegionChunk[8][8][4];
    Region(int id) {
        this.id = id;
        for (int x = 0; x < 8; x++) for (int y = 0; y < 8; y++) for (int z = 0; z < 4; z++)
            chunks[x][y][z] = new RegionChunk(java.util.Arrays.asList(((id >> 8) << 6) + (x << 3), ((id & 0xff) << 6) + (y << 3), z));
    }
    public RegionChunk[][][] getChunks() { return chunks; }
    public Tolerances getTolerances() { return new Tolerances(); }
    public class Tolerances {
        public Object remove(Object name) { oracle.Support.ev("Region.tolerances.remove", id, name); return null; }
    }
}
''',
    'core/game/world/map/RegionChunk.java': r'''package core.game.world.map;
public class RegionChunk {
    final java.util.List<Integer> base;
    RegionChunk(java.util.List<Integer> base) { this.base = base; }
    public void remove(core.game.node.entity.npc.NPC n) { oracle.Support.ev("RegionChunk.remove", base, n); }
    public void removePlayer(core.game.node.entity.player.Player p) { oracle.Support.ev("RegionChunk.removePlayer", base, p); }
}
''',
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
    npc = c.read(SERVER / 'core/game/node/entity/npc/NPC.java')
    entity = c.read(SERVER / 'core/game/node/entity/Entity.java')
    return {
        'ORIGINAL_NPC_1': c.method(npc, 'public NPC(int id) {'),
        'ORIGINAL_NPC_3': c.method(npc, 'protected NPC(int id, Location location, Direction direction) {'),
        'ORIGINAL_NPC_2': c.method(npc, 'public NPC(int id, Location location) {'),
        'ORIGINAL_NPC_CLEAR': c.method(npc, 'public void clear() {'),
        'ORIGINAL_NPC_TRANSFORM': c.method(npc, 'public NPC transform(int id) {'),
        'ORIGINAL_NPC_TRANSFORM_HP': c.method(npc, 'public NPC transformWithHpCarryover(int id) {'),
        'ORIGINAL_NPC_RETRANSFORM': c.method(npc, 'public void reTransform() {'),
        'ORIGINAL_ENTITY_INIT': c.method(entity, 'public void init() {'),
        'ORIGINAL_ENTITY_CLEAR': c.method(entity, 'public void clear() {'),
    }


def libraries():
    pin = json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))['sha256']
    name = 'json-simple-1.1.1.jar'
    candidates = [RUN / name] + sorted((ROOT / 'runs').glob(f'*-oracle/{name}'))
    jar = next((p for p in candidates if p.is_file() and hashlib.sha256(p.read_bytes()).hexdigest() == pin), None)
    if jar is None:
        raise RuntimeError(f'{name} matching its pin was not found in runs/*-oracle (this generator never downloads)')
    if jar != RUN / name:
        shutil.copyfile(jar, RUN / name)
    return [RUN / name]


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
DEFS = [
    dict(id=1, name='Man', size=1),
    dict(id=50, name='King Black Dragon', size=5),
    dict(id=6571, name='Gravestone', size=1),
    dict(id=6565, name='Gravestone', size=2),
    dict(id=7, name=None, size=0),
    dict(id=-1, name='Negative', size=-2),
]
HOME = [3222, 3218, 0]


def case(name, ops, sweep=False, **extra):
    return dict(name=name, defs=extra.pop('defs', DEFS), ops=ops, **extra), sweep


def cases():
    out = []
    ids = [1, 50, 6571, 7, -1, 99]  # 99 has no definition
    places = [HOME, None, [3199, 3207, 2], [3263, 3327, 3], [64, 0, 1]]
    # The three constructors.
    for i in ids:
        out.append(case(f'new1/{i}', [['new1', i]], sweep=i in (1, 99)))
        for p, place in enumerate(places):
            out.append(case(f'new2/{i}/{p}', [['new2', i, place]], sweep=(i, p) == (50, 0)))
            for direction in [None, 'SOUTH', 'NORTH_WEST', 'EAST']:
                out.append(case(f'new3/{i}/{p}/{direction}', [['new3', i, place, direction]], sweep=(i, p, direction) == (1, 2, 'EAST')))
    # The definition table changes between two lookups (the constructor makes two; transform one).
    for at in (1, 2, 3):
        for change in [dict(id=1, name='Changed', size=4), dict(id=1, absent=True), dict(id=50, name='Other', size=9)]:
            hooks = [dict(at=at, **{'def': change})]
            out.append(case(f'hook/new/{at}/{change}', [['new2', 1, HOME], ['new1', 1], ['new3', 1, None, 'WEST']], sweep=at == 1, hooks=hooks))
            out.append(case(f'hook/transform/{at}/{change}', [['new2', 50, HOME], ['transform', 0, 1], ['reTransform', 0], ['transformHp', 0, 1]], sweep=at == 3 and 'absent' in change, hooks=hooks))
    # Entity.init and Entity.clear for an NPC and a player, placed or not, in or out of the repository.
    for p, place in enumerate(places):
        out.append(case(f'init-clear/npc/{p}', [['new2', 1, place], ['entityInit', 0], ['add', 0], ['renderable', 0], ['clear', 0], ['clear', 0]], sweep=p in (0, 1)))
        out.append(case(f'init-clear/player/{p}', [['player', 'traceone', place], ['entityInit', 0], ['clear', 0], ['entityInit', 0]], sweep=p in (0, 1)))
        out.append(case(f'clear/unlisted/{p}', [['new2', 50, place], ['clear', 0]]))
        out.append(case(f'clear/moved/{p}', [['new2', 1, HOME], ['add', 0], ['move', 0, place], ['clear', 0]]))
    out.append(case('clear/no-behavior', [['new2', 1, HOME], ['add', 0], ['behavior', 0], ['clear', 0]], sweep=True))
    out.append(case('clear/second-of-three', [['new2', 1, HOME], ['new2', 50, [3230, 3230, 0]], ['new2', 6571, [3200, 3200, 1]], ['add', 0], ['add', 1], ['add', 2],
                                               ['renderable', 1], ['renderable', 2], ['clear', 1], ['new2', 1, HOME], ['add', 3], ['clear', 0], ['clear', 2], ['clear', 3]], sweep=True))
    out.append(case('repository/full', [['new2', 1, HOME], ['new2', 1, HOME], ['new2', 1, HOME], ['add', 0], ['add', 1], ['add', 2], ['clear', 2], ['clear', 0], ['add', 2]], capacity=2))
    out.append(case('repository/none', [['new2', 1, HOME], ['add', 0], ['clear', 0]], capacity=0))
    # transform: to another id, back to the original, to a missing definition, repeated, with each ordinal answer.
    for first in ids:
        for second in ids:
            ops = [['new2', first, HOME], ['transform', 0, second]]
            out.append(case(f'transform/{first}-{second}', ops, sweep=(first, second) in [(1, 50), (1, 1), (1, 99), (50, 7)]))
            if first != 99:
                out.append(case(f'transform/{first}-{second}/back', ops + [['transform', 0, first], ['reTransform', 0], ['transform', 0, second], ['reTransform', 0], ['reTransform', 0]],
                                sweep=(first, second) in [(1, 50), (6571, 6565)]))
                out.append(case(f'transformHp/{first}-{second}', [['new2', first, HOME], ['lifepoints', 0, 37], ['transformHp', 0, second], ['transformHp', 0, first]],
                                sweep=(first, second) in [(1, 50), (1, 99)]))
    for ordinals in [[5], [-1], [0], [5, -1, 7], [2147483647, -2147483648]]:
        out.append(case(f'transform/ordinal/{ordinals}', [['new2', 1, HOME], ['transform', 0, 1], ['transform', 0, 50], ['transform', 0, 1], ['reTransform', 0], ['transform', 0, 1]], ordinals=ordinals))
    # The definition table changes under a live NPC.
    out.append(case('transform/definition-replaced', [['new2', 1, HOME], ['def', dict(id=1, name='Woman', size=3)], ['transform', 0, 1], ['def', dict(id=1, absent=True)], ['transform', 0, 1], ['reTransform', 0],
                                                      ['transform', 0, 50], ['reTransform', 0], ['def', dict(id=1, name='Man', size=1)], ['reTransform', 0]], sweep=True))
    out.append(case('transform/cleared', [['new2', 6571, HOME], ['add', 0], ['clear', 0], ['transform', 0, 6565], ['entityInit', 0], ['add', 0], ['reTransform', 0], ['clear', 0]], sweep=True))
    out.append(case('transform/player-not-an-npc', [['player', 'traceone', HOME], ['transform', 0, 1], ['reTransform', 0]]))
    # A random walk over the operations.
    rng = random.Random(53531)
    for n in range(60):
        ops, count = [], 0
        for _ in range(rng.randrange(5, 16)):
            kind = rng.choice(['new1', 'new2', 'new3', 'player', 'entityInit', 'clear', 'transform', 'transformHp', 'reTransform', 'add', 'renderable', 'move', 'def', 'lifepoints']) if count else 'new2'
            who = rng.randrange(count) if count else 0
            place = rng.choice([[rng.randrange(3190, 3270), rng.randrange(3190, 3270), rng.randrange(4)], None])
            if kind == 'new1':
                ops.append(['new1', rng.choice(ids)])
                count += 1
            elif kind == 'new2':
                ops.append(['new2', rng.choice(ids[:5]), place])
                count += 1
            elif kind == 'new3':
                ops.append(['new3', rng.choice(ids[:5]), place, rng.choice([None, 'SOUTH', 'WEST', 'NORTH_EAST'])])
                count += 1
            elif kind == 'player':
                ops.append(['player', f'p{count}', place])
                count += 1
            elif kind in ('transform', 'transformHp'):
                ops.append([kind, who, rng.choice(ids)])
            elif kind == 'move':
                ops.append(['move', who, place])
            elif kind == 'def':
                i = rng.choice(ids)
                ops.append(['def', rng.choice([dict(id=i, absent=True), dict(id=i, name=f'N{n}', size=rng.randrange(-1, 6))])])
            elif kind == 'lifepoints':
                ops.append(['lifepoints', who, rng.randrange(-5, 200)])
            else:
                ops.append([kind, who])
        out.append(case(f'random/{n}', ops, sweep=n < 8, capacity=rng.choice([1, 3, 8]), ordinals=rng.choice([[5], [5, -1], [3]])))
    return out


EXPECTED_ERRORS = (None, 'Null', 'Binding', 'Index')


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
    summary = f'{len(rows)} original runtime NPC cases ({steps} operations, {coverage["recordedCalls"]} recorded calls, {len(failing)} failure cases)'
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('npc-runtime fixtures/provenance differ; inspect before regeneration')
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
