"""Reproduce original projectile path and combat line-of-sight observations."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import random
import subprocess
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
RUN = ROOT / 'runs/projectile-oracle'
SOURCE = ROOT / 'reference/server/Server/src/main'
spec = importlib.util.spec_from_file_location('compiler', ROOT / 'oracle/login/generate.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


def cases():
    start = [3207, 3216, 2]
    rows = []

    def add(dx, dy, **options):
        rows.append(dict(start=start, end=[start[0]+dx, start[1]+dy, 1], **options))

    for dx, dy in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)]:
        for flags in [0, -1, 0x2, 0x8, 0x20, 0x80, 0x100, 0x200, 0x20000, 0x1000000, 0x40000000]:
            add(dx, dy, defaultProjectile=flags)
            add(dx, dy, defaultWalking=flags)
    for dx, dy in [(4, 5), (-5, 4), (4, -5), (-4, -5), (4, 0), (0, -4)]:
        for direction in [-1, 0, 1]:
            add(dx, dy, flags=[[True, start[0]+direction, start[1]+1, 0x20000]])
        for call in [1, 2, 5, 10]:
            add(dx, dy, failAt=call)
        for call in [1, 4]:
            add(dx, dy, mutateAt=call, mutateTarget='end', replace=[3205, 3218, 0])
            add(dx, dy, mutateAt=call, mutateTarget='start', replace=[3206, 3215, 3],
                flags=[[False, 3207+max(-1, min(1, dx)), 3216+max(-1, min(1, dy)), 0x20000]])
    rng = random.Random(53062)
    for _ in range(40):
        flags = [[bool(rng.randrange(2)), start[0]+rng.randrange(-3, 5), start[1]+rng.randrange(-3, 5),
                  rng.choice([0, 2, 8, 32, 128, 256, 0x20000, -1])] for _ in range(30)]
        add(rng.randrange(-4, 5), rng.randrange(-4, 5), flags=flags)
    for size in [-1, 0, 1, 2, 3]:
        for target in [0, 1, 2]:
            for close in [False, True]:
                for dx, dy in [(1, 0), (2, 2), (-3, 1)]:
                    add(dx, dy, kind='combat', sourceSize=size, victimSize=target, close=close,
                        flags=[[True, 3208, 3216, 0x20000]])
    add(2, 1, kind='los', sourceSize=2, victimSize=1,
        flags=[[True, 3208, 3216, 0x20000]])
    add(2, 1, kind='los', sourceSize=2, victimSize=1, defaultProjectile=-1)
    rows.extend([
        dict(start=None, end=start), dict(start=start, end=None),
        dict(start=start, end=None, kind='combat', sourceSize=0),
        dict(start=start, end=None, kind='combat', sourceSize=1),
        dict(start=None, end=start, kind='combat', sourceSize=1, victimSize=1),
        dict(start=[16381, -16384, 3], end=[16383, -16382, 0]),
        dict(start=[-16381, 16383, 3], end=[-16384, 16381, 1]),
    ])
    return rows


def generate(check):
    read, run = c.read, c.run
    lock = json.loads(read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        repo = ROOT / 'reference' / part
        assert run(['git', '-C', repo, 'rev-parse', 'HEAD']).stdout.decode().strip() == lock[part]['commit']
        run(['git', '-C', repo, 'diff', '--quiet', 'HEAD', '--'])
    classes = RUN / 'classes'
    classes.mkdir(parents=True, exist_ok=True)
    jars = c.artifacts()
    std = next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    artifact = json.loads(read(ROOT / 'oracle/auth/json.lock.json'))
    jar = RUN / 'json-simple-1.1.1.jar'
    if not jar.exists() or hashlib.sha256(jar.read_bytes()).hexdigest() != artifact['sha256']:
        cached = ROOT / 'runs/pathfinding-oracle/json-simple-1.1.1.jar'
        if cached.is_file():
            data = cached.read_bytes()
        else:
            with urllib.request.urlopen(artifact['url'], timeout=60) as response:
                data = response.read()
        assert len(data) == artifact['size'] and hashlib.sha256(data).hexdigest() == artifact['sha256']
        jar.write_bytes(data)
    java, kotlin = [], []

    def adapter(name, content):
        path = RUN / 'adapters' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding='utf-8')
        (java if path.suffix == '.java' else kotlin).append(path)

    world = 'core/game/world/map/'
    for name in ['Direction.java', 'Point.java', 'path/Path.java', 'path/ProjectilePathfinder.java', 'path/ClipMaskSupplier.java']:
        path = SOURCE / (world + name)
        read(path)
        java.append(path)
    pathfinder = read(SOURCE / (world + 'path/Pathfinder.java'))
    decl = next(line.strip() for line in pathfinder.splitlines() if 'ProjectilePathfinder PROJECTILE' in line)
    adapter(world + 'path/Pathfinder.java', 'package core.game.world.map.path;import core.game.world.map.Location;public abstract class Pathfinder{' + decl + 'public abstract Path find(Location a,int s,Location b,int sx,int sy,int r,int t,int f,boolean n,ClipMaskSupplier c);}')
    location = read(SOURCE / (world + 'Location.kt'))
    transform = next(line.strip() for line in location.splitlines() if 'fun transform(diffX: Int, diffY: Int, z: Int)' in line)
    adapter(world + 'Location.kt', '''package core.game.world.map
import core.game.node.Node
class Location(x: Int,y: Int,z: Int): Node() {
    var hash=LocationHash(x,y,(z%4+4)%4)
    val x get()=hash.x
    val y get()=hash.y
    val z get()=hash.z
    override fun getLocation(): Location = this
    fun replace(x:Int,y:Int,z:Int){hash=LocationHash(x,y,(z%4+4)%4)}
''' + transform + '\n@JvmInline ' + c.method(location, 'value class LocationHash(') + '}')
    read(SOURCE / 'core/game/node/Node.java')
    adapter('core/game/node/Node.java', 'package core.game.node;import core.game.world.map.Location;public class Node{public int size=1;public Location location;public int size(){return size;}public Location getLocation(){return location;}}')
    adapter('core/game/node/entity/Entity.java', 'package core.game.node.entity;public class Entity extends core.game.node.Node{public Locks getLocks(){return new Locks();}public Walking getWalkingQueue(){return new Walking();}public static class Locks{public boolean isMovementLocked(){return false;}}public static class Walking{public void reset(){}public void addPath(int x,int y){}}}')
    adapter(world + 'RegionManager.java', 'package core.game.world.map;public class RegionManager{public static int getClippingFlag(int z,int x,int y){return oracle.Support.flag(false,z,x,y);}public static int getProjectileFlag(int z,int x,int y){return oracle.Support.flag(true,z,x,y);}}')
    combat = read(SOURCE / 'core/game/node/entity/combat/CombatSwingHandler.kt')
    adapter('core/game/node/entity/combat/CombatSwingHandler.kt', '''package core.game.node.entity.combat
import core.game.node.Node
import core.game.world.map.RegionManager
import core.game.world.map.path.Pathfinder.PROJECTILE
class CombatSwingHandler {companion object {@JvmStatic
''' + c.method(combat, 'fun isProjectileClipped(entity: Node, victim: Node?, checkClose: Boolean)') + '}}')
    api = read(SOURCE / 'core/api/ContentAPI.kt')
    adapter('core/api/ContentAPI.kt', '''package core.api
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.combat.CombatSwingHandler
''' + c.method(api, 'fun hasLineOfSight(entity: Entity, target: Node)'))
    read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    deps = os.pathsep.join(map(str, [std, jar]))
    cp = os.pathsep.join(map(str, [classes, std, jar]))
    run(['java', '-Xmx512m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect', '-jvm-target', '11', '-classpath', deps, '-d', classes, *kotlin, *java])
    run(['javac', '--release', '11', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    rows = cases()
    exe = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    process = subprocess.run([exe, '-cp', cp, 'oracle.Support'], input=''.join(json.dumps(row) + '\n' for row in rows).encode(), capture_output=True, cwd=ROOT, timeout=60)
    if process.returncode:
        raise RuntimeError(process.stderr.decode(errors='replace'))
    observations = [json.loads(line) for line in process.stdout.splitlines()]
    assert len(observations) == len(rows)
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in observations).encode()
    read(Path(__file__))
    read(ROOT / 'oracle/login/generate.py')
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm='sha256, CR removed', scope='Unchanged ProjectilePathfinder, Point, Path and Direction, and verbatim isProjectileClipped/hasLineOfSight with packed LocationHash and transform. Minimal node/location and region-flag adapters record ordered queries and failure prefixes. Does not cover all combat or a reentrant JVM singleton.')
    folder = ROOT / 'fixtures/projectile'
    if check:
        assert (folder / 'oracle.jsonl').read_bytes() == data
        assert json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) == provenance
        print(f'{len(rows)} original projectile and line-of-sight scenarios reproduced.')
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'oracle.jsonl').write_bytes(data)
        (folder / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {len(rows)} original projectile and line-of-sight scenarios.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
