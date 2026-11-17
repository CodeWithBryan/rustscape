"""Original entity-script host helpers; the complete script kernel has its own corpus."""
import argparse
import hashlib
import importlib.util
import itertools
import json
import os
import pathlib
import subprocess
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
HERE = pathlib.Path(__file__).parent
RUN = ROOT / 'runs/script-runtime-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
spec = importlib.util.spec_from_file_location('compiler', ROOT / 'oracle/login/generate.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
read, run = c.read, c.run


def requests():
    rows = []
    for opened, chatbox in itertools.product([None, -1, 0, 137, 372, 421, 762, 2147483647], repeat=2):
        rows.append(dict(op='modal', opened=opened, chatbox=chatbox))
    values = [-2147483648, -1, 0, 1, 2147483647]
    for tick, delay, stun, movement in itertools.product(values, repeat=4):
        rows.append(dict(op='clocks', tick=tick, delay=delay, stun=stun, movement=movement, length=10))
    for length in [0, 1, 6]:
        for tick in values:
            rows.append(dict(op='clocks', tick=tick, delay=0, stun=0, movement=0, length=length))
    locations = [[0, 0, 0], [-1, 1, -1], [16383, -16384, 3], [-16384, 16383, 4], [2147483647, -2147483648, 2147483647]]
    for loc, size in itertools.product(locations, [-2147483648, -1, 0, 1, 2, 3, 16384, 2147483647]):
        rows.append(dict(op='center', location=loc, size=size))
    dimensions = [[-1, 5], [0, 0], [1, 2], [2, 3], [2147483647, 2147483647], [-2147483648, 2], [16384, 3], [3, -2147483648]]
    for loc, dims, direction in itertools.product(locations, dimensions, range(8)):
        rows.append(dict(op='sceneryCenter', location=loc, dimensions=dims, direction=direction))
    return rows


def generate(check):
    lock = json.loads(read(ROOT / 'upstream.lock.json'))
    for part in ['server', 'client']:
        path = ROOT / 'reference' / part
        assert run(['git', '-C', path, 'rev-parse', 'HEAD']).stdout.decode().strip() == lock[part]['commit']
        run(['git', '-C', path, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(exist_ok=True)
    classes = RUN / 'classes'
    classes.mkdir(exist_ok=True)
    jars = c.artifacts()
    std = next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    artifact = json.loads(read(ROOT / 'oracle/auth/json.lock.json'))
    jar = RUN / 'json-simple-1.1.1.jar'
    if not jar.exists() or hashlib.sha256(jar.read_bytes()).hexdigest() != artifact['sha256']:
        with urllib.request.urlopen(artifact['url'], timeout=60) as r:
            data = r.read()
        assert len(data) == artifact['size'] and hashlib.sha256(data).hexdigest() == artifact['sha256']
        jar.write_bytes(data)
    java, kotlin = [], []

    def adapter(name, text):
        path = RUN / 'adapters' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')
        (kotlin if path.suffix == '.kt' else java).append(path)

    entity = read(SERVER / 'core/game/node/entity/Entity.java')
    player = read(SERVER / 'core/game/node/entity/player/Player.java')
    manager = read(SERVER / 'core/game/node/entity/player/link/InterfaceManager.java')
    component = read(SERVER / 'core/game/component/Component.java')
    node = read(SERVER / 'core/game/node/Node.java')
    scenery = read(SERVER / 'core/game/node/scenery/Scenery.java')
    location = read(SERVER / 'core/game/world/map/Location.kt')
    api = read(SERVER / 'core/api/ContentAPI.kt')
    adapter('core/game/node/entity/Entity.java', 'package core.game.node.entity;import core.game.world.GameWorld;public class Entity{public int[]clocks=new int[10];public Scripts scripts=new Scripts();public static class Scripts{public int delay;public int getDelay(){return delay;}}' + c.method(entity, 'public boolean delayed()') + '}')
    adapter('core/game/node/entity/player/Player.java', 'package core.game.node.entity.player;import core.game.component.Component;import core.game.node.entity.player.link.InterfaceManager;public class Player extends core.game.node.entity.Entity{public InterfaceManager interfaceManager=new InterfaceManager();' + c.method(player, 'public boolean hasModalOpen()') + '}')
    adapter('core/game/component/Component.java', 'package core.game.component;public class Component{public int id;public Component(int value){id=value;}' + c.method(component, 'public int getId()') + '}')
    adapter('core/game/node/entity/player/link/InterfaceManager.java', 'package core.game.node.entity.player.link;import core.game.component.Component;public class InterfaceManager{public static final int DEFAULT_CHATBOX=137;public Component opened,chatbox;public Component getOpened(){return opened;}public Component getChatbox(){return chatbox;}' + c.method(manager, 'public boolean isOpened()') + c.method(manager, 'public boolean hasChatbox()') + '}')
    adapter('core/game/world/GameWorld.java', 'package core.game.world;public class GameWorld{public static int ticks;public static int getTicks(){return ticks;}}')
    adapter('core/game/node/Node.java', 'package core.game.node;import core.game.world.map.Location;public class Node{public Location location;public int size=1;' + c.method(node, 'public Location getCenterLocation()') + '}')
    adapter('core/game/node/scenery/Scenery.java', 'package core.game.node.scenery;import core.game.world.map.Location;public class Scenery extends core.game.node.Node{public Direction direction=new Direction();public Definition definition=new Definition();public static class Direction{public int value;public int toInteger(){return value;}}public static class Definition{public int sizeX,sizeY;}' + c.method(scenery, 'public int getSizeX()') + c.method(scenery, 'public int getSizeY()') + c.method(scenery, 'public Location getCenterLocation()') + '}')
    transform = next(line.strip() for line in location.splitlines() if 'fun transform(diffX: Int, diffY: Int, z: Int)' in line)
    adapter('core/game/world/map/Location.kt', '''package core.game.world.map
class Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x get()=hash.x;val y get()=hash.y;val z get()=hash.z;
''' + transform + '\n@JvmInline ' + c.method(location, 'value class LocationHash(') + '}')
    adapter('core/api/API.kt', '''package core.api
import core.game.node.entity.Entity
import core.game.world.GameWorld
import core.game.interaction.Clocks
fun getWorldTicks()=GameWorld.ticks
''' + '\n'.join(c.method(api, method) for method in ['fun isStunned(entity: Entity)', 'fun finishedMoving(entity: Entity)']))
    path = SERVER / 'core/game/interaction/Clocks.kt'
    read(path)
    kotlin.append(path)
    read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    cp = os.pathsep.join(map(str, [classes, std, jar]))
    run(['java', '-Xmx512m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-no-stdlib', '-no-reflect', '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, [std, jar])), '-d', classes, *kotlin, *java])
    run(['javac', '--release', '11', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    rows = requests()
    data = ''.join(json.dumps(row, sort_keys=True) + '\n' for row in rows).encode()
    result = subprocess.run(list(map(str, [c.JAVA_HOME / 'bin/java.exe' if c.JAVA_HOME and os.name == 'nt' else c.JAVA_HOME / 'bin/java' if c.JAVA_HOME else 'java', '-cp', cp, 'oracle.Support'])), input=data, capture_output=True, cwd=ROOT, timeout=60)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace'))
    observations = [json.loads(line) for line in result.stdout.splitlines() if line]
    assert len(observations) == len(rows)
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in observations).encode()
    read(pathlib.Path(__file__))
    read(ROOT / 'oracle/login/generate.py')
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())), caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(), sourceHashAlgorithm='sha256, CR removed', scope='Verbatim Entity.delayed, Player.hasModalOpen, InterfaceManager.isOpened/hasChatbox, Component.getId, Node center and Scenery center/dimension methods; verbatim isStunned/finishedMoving and complete unchanged Clocks.kt. Packed LocationHash/transform are original. Entity/UI/node construction, field inputs and GameWorld clock are explicit adapters. This covers host helper bindings, not complete Entity/Player/scenery or tick/content/lock/projectile dispatch.')
    folder = ROOT / 'fixtures/script-runtime'
    if check:
        assert (folder / 'oracle.jsonl').read_bytes() == data
        assert json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) == provenance
        print(f'{len(rows)} original script runtime helper scenarios reproduced.')
    else:
        folder.mkdir(exist_ok=True)
        (folder / 'oracle.jsonl').write_bytes(data)
        (folder / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {len(rows)} original script runtime helper scenarios.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
