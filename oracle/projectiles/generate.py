"""Run the original Projectile and ProjectileUpdateFlag on JDK 11 and record what they do.

Compiled unchanged: core/game/node/entity/impl/Projectile.java, core/game/world/update/flag/chunk/ProjectileUpdateFlag.java,
core/game/world/update/flag/UpdateFlag.java, core/game/world/map/Location.kt and core/api/utils/Vector.kt. Original text
inserted into adapters at generation time: Node.getCenterLocation and IoBuffer.put(int)/putShort(int). Everything else
(Node's getters, Entity/NPC/Player, RegionManager.getRegionChunk, RegionChunk.flag) is a recording adapter answering from
the case. Each case runs in a class loader of its own (oracle/content-api/Boot.java). Java/Kotlin are test oracles only;
--check never rewrites a fixture.
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
RUN = ROOT / 'runs/projectiles-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
FIXTURES = ROOT / 'fixtures/projectiles'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


c = load('compiler', ROOT / 'oracle/login/generate.py')

UNCHANGED = [
    'core/game/node/entity/impl/Projectile.java',
    'core/game/world/update/flag/chunk/ProjectileUpdateFlag.java',
    'core/game/world/update/flag/UpdateFlag.java',
    'core/game/world/map/Location.kt',
    'core/api/utils/Vector.kt',
]
SCOPE = (
    'Projectile.java, ProjectileUpdateFlag.java, UpdateFlag.java, Location.kt and Vector.kt compiled unchanged on JDK 11 '
    'with Kotlin 1.8.20; Node.getCenterLocation and IoBuffer.put(int)/putShort(int) inserted verbatim. Node.size, '
    'Node.getLocation, Node.getIndex, RegionManager.getRegionChunk and RegionChunk.flag are recording adapters; '
    'field reads are not recorded. Every factory, transform, copy, send, setter and the flag write (bytes, failed '
    'prefixes, the target index and end location read at write time) with injected failures of every recorded call. '
    'Does not cover RegionChunk.update ordering (chunk-sync-oracle) or the client.'
)

ADAPTERS = {
    'core/game/node/Node.java': r'''package core.game.node;
import core.game.world.map.Location;
public class Node {
    protected String name;
    protected Location location;
    protected int index;
    protected int size = 1;
    public core.game.interaction.DestinationFlag destinationFlag;
    public Node() {}
    public Node(String name, Location location) { this.name = name; this.location = location; }
    public Location getLocation() { oracle.Support.ev("Node.getLocation", this); return location; }
    public int size() { oracle.Support.ev("Node.size", this); return size; }
    public int getIndex() { oracle.Support.ev("Node.getIndex", this); return index; }
    /* ORIGINAL_CENTER */
    public Location getLocation0() { return location; }
    public void oracleLocation(Location l) { location = l; }
    public void oracleIndex(int i) { index = i; }
    public void oracleSize(int s) { size = s; }
}
''',
    'core/game/node/entity/Entity.java': 'package core.game.node.entity;\npublic class Entity extends core.game.node.Node {}\n',
    'core/game/node/entity/npc/NPC.java': 'package core.game.node.entity.npc;\npublic class NPC extends core.game.node.entity.Entity {}\n',
    'core/game/node/entity/player/Player.java': 'package core.game.node.entity.player;\npublic class Player extends core.game.node.entity.Entity {}\n',
    'core/game/interaction/DestinationFlag.java': 'package core.game.interaction;\npublic class DestinationFlag { public static final DestinationFlag LOCATION = new DestinationFlag(); }\n',
    'core/net/packet/IoBuffer.java': r'''package core.net.packet;
import java.nio.ByteBuffer;
public class IoBuffer {
    private ByteBuffer buf = ByteBuffer.allocate(256);
    /* ORIGINAL_PUT */
    /* ORIGINAL_PUT_SHORT */
    public java.util.List<Object> oracleBytes() {
        java.util.List<Object> out = new java.util.ArrayList<>();
        for (int i = 0; i < buf.position(); i++) out.add(buf.get(i) & 0xff);
        return out;
    }
}
''',
    'core/game/world/map/RegionManager.java': r'''package core.game.world.map;
public class RegionManager {
    public static Region forId(int id) { oracle.Support.ev("RegionManager.forId", id); return new Region(); }
    public static Location getTeleportLocation(Location l, int r) { throw new UnsupportedOperationException(); }
    public static RegionChunk getRegionChunk(Location l) { oracle.Support.ev("RegionManager.getRegionChunk", l); return new RegionChunk(l); }
}
''',
    'core/game/world/map/Region.java': 'package core.game.world.map;\npublic class Region { public RegionChunk[][][] getChunks() { return null; } }\n',
    'core/game/world/map/RegionChunk.java': r'''package core.game.world.map;
public class RegionChunk implements oracle.Support.Described {
    final Location base;
    public RegionChunk(Location base) { this.base = base; }
    public Object describe() { return base; }
    public void flag(core.game.world.update.flag.UpdateFlag<?> flag) { oracle.Support.flagged(this, flag); }
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
    node = c.read(SERVER / 'core/game/node/Node.java')
    io = c.read(SERVER / 'core/net/packet/IoBuffer.java')
    return {
        'ORIGINAL_CENTER': c.method(node, 'public Location getCenterLocation() {'),
        'ORIGINAL_PUT': c.method(io, 'public IoBuffer put(int val) {'),
        'ORIGINAL_PUT_SHORT': c.method(io, 'public IoBuffer putShort(int val) {'),
    }


def libraries():
    found = []
    pin = json.loads(c.read(ROOT / 'oracle/auth/json.lock.json'))['sha256']
    name = 'json-simple-1.1.1.jar'
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
WORLD = [
    dict(kind='player', size=1, loc=[3222, 3218, 0], index=5),       # e0
    dict(kind='npc', size=1, loc=[3227, 3214, 0], index=12),          # e1
    dict(kind='npc', size=3, loc=[3214, 3229, 1], index=300),         # e2
    dict(kind='entity', size=2, loc=[3201, 3199, 0], index=7),        # e3
    dict(kind='player', size=1, loc=None, index=1),                   # e4: never placed
    dict(kind='npc', size=0, loc=[3199, 3207, 2], index=0),           # e5
    dict(kind='npc', size=-3, loc=[3208, 3200, 3], index=2047),       # e6
    dict(kind='player', size=1, loc=[-16384, 16383, 3], index=2000),  # e7
    dict(kind='npc', size=5, loc=[16383, -16384, 0], index=32767),    # e8
    dict(kind='npc', size=2, loc=[3223, 3218, 0], index=-1),          # e9
]
PEOPLE = list(range(len(WORLD)))


def case(name, ops, entities=None, sweep=False):
    return dict(name=name, entities=entities or WORLD, ops=ops), sweep


def cases():
    out = []
    pairs = [(0, 1), (1, 0), (2, 0), (0, 2), (3, 1), (5, 0), (6, 2), (7, 8), (8, 7), (9, 0), (0, 0)]
    nulls = [(-1, 0), (0, -1), (-1, -1), (4, 0), (0, 4)]
    # Every factory overload, sent and written.
    for s, v in pairs + nulls:
        for arity in (3, 5, 6, 7, 8, 9):
            extra = [43, 31, 51, 70, 16, 64][:arity - 3]
            ops = [['create', arity, s, v, 10 + arity] + extra, ['sendStatic', 0], ['write', 0]]
            out.append(case(f'create{arity}/{s}-{v}', ops, sweep=(s, v) in [(0, 1), (2, 0), (-1, 0)]))
        out.append(case(f'magic/{s}-{v}', [['magic', s, v, 156, 43, 31, 51, 16], ['send', 0], ['write', 0]], sweep=(s, v) == (0, 1)))
        out.append(case(f'ranged/{s}-{v}', [['ranged', s, v, 10, 43, 31, 41, 15], ['send', 0], ['write', 0]], sweep=(s, v) == (2, 0)))
        out.append(case(f'getSpeed/{s}-{v}', [['getSpeed', s, [3230, 3230, 0]], ['getLocation', s], ['getSpeed', v, None]]))
    # Location-based projectiles.
    starts = [[3222, 3218, 0], [3199, 3207, 2], [-16384, 16383, 3], [16383, -16384, 1], None, [7, 15, 0], [-1, -9, 2]]
    ends = [[3230, 3210, 0], [3199, 3207, 2], [16383, -16384, 0], None, [-129, 128, 3]]
    for i, s in enumerate(starts):
        for j, e in enumerate(ends):
            out.append(case(f'createLoc/{i}-{j}', [['createLoc', s, e, 1218, 0, 0, 1, 50, 0, 0], ['send', 0], ['write', 0]],
                            sweep=(i, j) == (0, 0)))
    # The empty constructor.
    out.append(case('empty', [['empty'], ['send', 0]]))
    out.append(case('empty/write', [['empty'], ['set', 0, 'sourceLocation', [3222, 3218, 0]], ['send', 0], ['write', 0]]))
    out.append(case('empty/victim', [['empty'], ['set', 0, 'sourceLocation', [3222, 3218, 0]], ['set', 0, 'victim', 1], ['send', 0], ['write', 0]]))
    # transform (entity and location targets, the NPC distance rule, base and modified speeds).
    for s in [0, 1, 2, 3, 5, 6, 8, -1, 4]:
        for v in [0, 1, 2, -1, 4]:
            out.append(case(f'transform/{s}-{v}', [['empty'], ['transform', 0, s, v], ['send', 0], ['write', 0]],
                            sweep=(s, v) in [(2, 0), (0, 1)]))
        for npc, base, modified in [(True, 46, 5), (False, 46, 5), (True, 0, 0), (False, -100, 7), (True, 2147483647, 3), (False, -2147483648, -1)]:
            out.append(case(f'transform5/{s}/{npc}/{base}/{modified}', [['create', 9, 0, 1, 99, 1, 2, 3, 4, 5, 6], ['transform5', 0, s, 1, npc, base, modified], ['write', 0]]))
            out.append(case(f'transformLoc5/{s}/{npc}/{base}/{modified}', [['create', 9, 0, 1, 99, 1, 2, 3, 4, 5, 6], ['transformLoc5', 0, s, [3200, 3200, 0], npc, base, modified], ['sendStatic', 0], ['write', 0]]))
        for target in [[3230, 3210, 0], None, ['ent', 1], ['ent', 4]]:
            out.append(case(f'transformLoc/{s}/{target}', [['empty'], ['transformLoc', 0, s, target], ['send', 0], ['write', 0]],
                            sweep=(s, target) == (2, [3230, 3210, 0])))
    # send(source, victim, p) and send(source, location, p).
    for s, v in pairs[:6] + nulls[:3]:
        out.append(case(f'sendTo/{s}-{v}', [['create', 9, 3, 2, 50, 10, 20, 30, 40, 50, 60], ['sendTo', s, v, 0], ['write', 0]], sweep=(s, v) == (0, 1)))
        out.append(case(f'sendAt/{s}-{v}', [['create', 9, 3, 2, 50, 10, 20, 30, 40, 50, 60], ['sendAt', s, [3230, 3209, 0], 0], ['write', 0]], sweep=(s, v) == (2, 0)))
    # copy (the template's numbers, the speed multiplier over plain locations, the template's distance).
    for s, v in pairs + nulls:
        for multiplier in [0.0, 1.0, 1.5, -2.25, 1e300, -1e300, 0.1]:
            out.append(case(f'copy/{s}-{v}/{multiplier}', [['create', 9, 2, 0, 405, 40, 36, 41, 46, 5, 11], ['copy', 0, s, v, multiplier], ['send', 1], ['write', 0]],
                            sweep=(s, v, multiplier) in [((0, 1), 1.5)] or ((s, v) == (0, 1) and multiplier == 1.5)))
    # Fields read when the flag is written: the target index, the victim's location, the start location identity.
    out.append(case('write/late-index', [['create', 3, 0, 1, 10], ['send', 0], ['index', 1, 99], ['write', 0], ['index', 1, -5], ['write', 0]]))
    out.append(case('write/late-victim-move', [['create', 3, 0, 1, 10], ['send', 0], ['move', 1, [3290, 3150, 0]], ['write', 0], ['move', 1, None], ['write', 0]]))
    out.append(case('write/late-victim-setLoc', [['create', 3, 0, 1, 10], ['send', 0], ['setLoc', 1, 3100, 3300, 2], ['write', 0]]))
    out.append(case('write/late-setters', [['create', 3, 0, 1, 10], ['send', 0], ['set', 0, 'speed', 70000], ['set', 0, 'angle', -1], ['set', 0, 'distance', 300],
                                           ['set', 0, 'projectileId', -2], ['set', 0, 'startHeight', 256], ['set', 0, 'endHeight', -129], ['set', 0, 'startDelay', 65536], ['write', 0]]))
    out.append(case('write/late-source-location', [['create', 3, 0, 1, 10], ['send', 0], ['set', 0, 'sourceLocation', None], ['write', 0], ['set', 0, 'sourceLocation', [-9, 17, 1]], ['write', 0]]))
    out.append(case('write/late-victim', [['create', 3, 0, 1, 10], ['send', 0], ['set', 0, 'victim', None], ['write', 0], ['set', 0, 'victim', 2], ['write', 0], ['set', 0, 'endLocation', [3200, 3200, 0]], ['write', 0], ['set', 0, 'victim', None], ['write', 0]]))
    out.append(case('write/aliased-end', [['empty'], ['transformLoc', 0, 0, ['ent', 1]], ['send', 0], ['setLoc', 1, 3240, 3200, 0], ['write', 0]]))
    out.append(case('write/aliased-start', [['createLoc', ['ent', 3], [3230, 3210, 0], 1, 2, 3, 4, 5, 6, 7], ['send', 0], ['setLoc', 3, 3260, 3217, 1], ['write', 0]]))
    out.append(case('write/setter-source', [['create', 3, 0, 1, 10], ['set', 0, 'source', 2], ['send', 0], ['write', 0], ['transform', 0, 2, 0], ['write', 0]]))
    out.append(case('write/reused-object', [['create', 3, 0, 1, 10], ['send', 0], ['sendTo', 2, 0, 0], ['sendAt', 1, [3230, 3230, 0], 0], ['write', 0], ['write', 1], ['write', 2],
                                            ['compare', 0, 1], ['equals', 1, 2]]))
    out.append(case('write/two-chunks', [['create', 3, 0, 1, 10], ['create', 3, 2, 0, 11], ['send', 1], ['send', 0], ['write', 0], ['write', 1], ['compare', 1, 0]]))
    out.append(case('write/end-location-entity-alias', [['createLoc', [3222, 3218, 0], ['ent', 1], 1, 2, 3, 4, 5, 6, 7], ['set', 0, 'victim', 0], ['send', 0], ['move', 1, [3000, 3000, 0]], ['write', 0], ['setLoc', 1, 3001, 3002, 0], ['write', 0]]))
    # Chained operations: transform keeps the object, copy makes a new one from the transformed fields.
    out.append(case('chain/transform-copy', [['create', 9, 0, 1, 405, 40, 36, 41, 46, 5, 11], ['transform', 0, 2, 0], ['copy', 0, 0, 2, 2.5], ['transformLoc', 1, 8, ['src', 0]], ['send', 0], ['send', 1], ['write', 0], ['write', 1]], sweep=True))
    out.append(case('chain/end-then-entity', [['createLoc', [3222, 3218, 0], [3230, 3210, 0], 1, 2, 3, 4, 5, 6, 7], ['transform', 0, 1, 0], ['send', 0], ['write', 0]]))
    # A random walk over the operations.
    rng = random.Random(53530)
    for n in range(60):
        ops = [['create', 3, rng.choice(PEOPLE[:4]), rng.choice(PEOPLE[:4]), rng.randrange(2000)]]
        slots, flags = 1, 0
        for _ in range(rng.randrange(4, 12)):
            kind = rng.choice(['transform', 'transform5', 'transformLoc', 'copy', 'send', 'sendTo', 'sendAt', 'set', 'write', 'move', 'index', 'size', 'magic', 'ranged'])
            p = rng.randrange(slots)
            who = rng.choice(PEOPLE + [-1])
            other = rng.choice(PEOPLE + [-1])
            place = rng.choice([[rng.randrange(3190, 3240), rng.randrange(3190, 3240), rng.randrange(4)], None, ['ent', rng.randrange(len(WORLD))], ['src', p]])
            if kind == 'transform':
                ops.append(['transform', p, who, other])
            elif kind == 'transform5':
                ops.append(['transform5', p, who, other, rng.random() < 0.5, rng.randrange(-50, 100), rng.randrange(-5, 12)])
            elif kind == 'transformLoc':
                ops.append(['transformLoc', p, who, place])
            elif kind == 'copy':
                ops.append(['copy', p, who, other, rng.choice([0.0, 0.5, 2.0, 7.25])])
                slots += 1
            elif kind in ('magic', 'ranged'):
                ops.append([kind, who, other, rng.randrange(2000), rng.randrange(100), rng.randrange(100), rng.randrange(100), rng.randrange(30)])
                slots += 1
            elif kind == 'send':
                ops.append([rng.choice(['send', 'sendStatic']), p])
                flags += 1
            elif kind == 'sendTo':
                ops.append(['sendTo', who, other, p])
                flags += 1
            elif kind == 'sendAt':
                ops.append(['sendAt', who, place, p])
                flags += 1
            elif kind == 'set':
                field = rng.choice(['source', 'sourceLocation', 'victim', 'projectileId', 'startHeight', 'endHeight', 'startDelay', 'speed', 'angle', 'distance', 'endLocation'])
                value = who if field in ('source', 'victim') else place if field in ('sourceLocation', 'endLocation') else rng.randrange(-300, 70000)
                ops.append(['set', p, field, value])
            elif kind == 'write' and flags:
                ops.append(['write', rng.randrange(flags)])
            elif kind == 'move':
                ops.append(['move', rng.randrange(len(WORLD)), place if place is None or place[0] not in ('src',) else None])
            elif kind == 'index':
                ops.append(['index', rng.randrange(len(WORLD)), rng.randrange(-3, 40000)])
            elif kind == 'size':
                ops.append(['size', rng.randrange(len(WORLD)), rng.randrange(-2, 6)])
        for f in range(flags):
            ops.append(['write', f])
        out.append(case(f'random/{n}', ops, sweep=n < 6))
    return out


EXPECTED_ERRORS = (None, 'Null', 'Binding', 'Index')


def generate(check):
    lock, classpath, classes = build()
    planned = cases()
    plain = [x for x, _ in planned]
    rows = execute(classpath, classes, plain)
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
    summary = f'{len(rows)} original projectile cases ({steps} operations, {coverage["recordedCalls"]} recorded calls, {len(failing)} failure cases)'
    if check:
        if (FIXTURES / 'oracle.jsonl').read_bytes() != data or json.loads((FIXTURES / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('projectiles fixtures/provenance differ; inspect before regeneration')
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
