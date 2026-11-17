"""Run the unchanged pinned scenery-click dispatcher against mutable world services."""
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
HERE = pathlib.Path(__file__).resolve().parent
RUN = ROOT / 'runs/scenery-action-oracle'
SERVER = ROOT / 'reference/server/Server/src/main'
spec = importlib.util.spec_from_file_location('compiler', ROOT / 'oracle/login/generate.py')
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
read, run = c.read, c.run


def op(name, **kw):
    return dict(op=name, **kw)


def requests():
    rows = []

    def add(**kw):
        rows.append(dict(actions=[op('process')], **kw))

    for id, lookup, active in itertools.product(
            [-2147483648, -1, 0, 2420, 2421, 2422, 2423, 2424, 2425, 2426,
             2427, 6897, 6898, 6899, 6900, 12348, 2147483647],
            [None, 'base', 'other'], [False, True]):
        add(id=id, lookup=lookup, objects={'base': {'id': id, 'active': active}})
    for index, location in itertools.product([-2147483648, -1, 0, 1, 2, 3, 4, 5, 2147483647],
                                             [[3207, 3216, 0], [3207, 3216, 3], None]):
        add(index=index, location=location)
    for direction, config, varbit in itertools.product(range(8), [-1, 0, 7], [None, 101]):
        add(objects={'base': {'child': 'other', 'direction': direction, 'configFile': config,
                              'configId': -2147483648}}, varbit=varbit)
    for listeners, plugin, null_child, null_interaction in itertools.product([False, True], repeat=4):
        add(listeners=listeners, plugin=plugin,
            objects={'base': {'child': None if null_child else 'other'},
                     'other': {'interaction': None if null_interaction else 'other'}})
    for name in [None, '', 'Open', 'go-through', 'OPEN', 'İΣ😀', 'null']:
        for index in [-2147483648, -1, 0, 4, 2147483647]:
            add(options={'base': [{'name': name, 'index': index}]})
    for null in ['player', 'definition']:
        add(**({'player': None} if null == 'player' else
               {'objects': {'base': {'child': 'other', 'definition': None}}}))
    # Every external callback may fail or reenter; observe the surviving prefix.
    events = [('lookup', 1), ('child', 1), ('interaction', 1), ('interaction', 2),
              ('option', 1), ('debug', 1), ('debug', 2), ('debug', 3), ('debug', 4),
              ('debug', 5), ('varbit', 1), ('scripts', 1), ('listeners', 1),
              ('plugin', 1), ('handle', 1), ('construct', 1), ('invalid', 1), ('minimap', 1)]
    mutations = [op('fail'), op('set', lookup=None, listeners=True),
                 op('set', objects={'base': {'id': 3, 'location': [3220, 3221, 2], 'direction': 7},
                                    'other': {'id': 99, 'interaction': 'base'}}),
                 op('set', options={'other': [{'name': 'Replacement', 'index': 7}]}),
                 op('set', varbit=None, objects={'base': {'configId': 1179}}),
                 op('process', id=6898, index=0)]
    for (event, at), mutation in itertools.product(events, mutations):
        kw = dict(objects={'base': {'child': 'other', 'configFile': 7}},
                  hooks=[dict(event=event, at=at, actions=[mutation])])
        if event == 'construct':
            kw['id'] = 2424
        elif event in ['invalid', 'minimap']:
            kw['lookup'] = None
        add(**kw)
    # Captured wrapper status and option survive later identity/array changes.
    add(objects={'base': {'child': 'other'}}, hooks=[dict(event='debug', actions=[
        op('set', objects={'other': {'id': 12348}})])])
    add(hooks=[dict(event='debug', actions=[op('set', objects={'base': {'child': 'other', 'id': 50}})])])
    add(hooks=[dict(event='option', actions=[op('set', objects={'base': {'interaction': 'other'}})])])
    add(hooks=[dict(event='scripts', actions=[op('set', options={'base': [{'name': 'Changed', 'index': 8}]})])])
    # Family-crest construction uses a second, late player-plane read.
    for id in [2422, 2424, 2426, 6898, 6899]:
        for value in [None, [3207, 3216, 2]]:
            add(id=id, hooks=[dict(event='lookup', actions=[op('set', location=value)])])
    rows.append(dict(objects={'base': {'child': 'other'}}, actions=[op('process'),
        op('set', lookup=None), op('process'), op('set', lookup='base', listeners=True),
        op('process'), op('set', listeners=False, plugin=True), op('process')]))
    for name, option, shape, active in itertools.product(
            ['Door', 'Gate', 'Fence', 'Wall', 'Exit', 'Entrance', 'Mithril door',
             'Drawers', 'Wardrobe', 'cupboard', 'Cupboard', 'trapdoor', 'trap door',
             'drawers wardrobe cupboard', 'Stone', '', None],
            ['open', 'go-through', 'close', 'shut', 'Open', 'search', None], [0, 9, 10], [False, True]):
        rows.append(dict(objects={'base': {'name': name, 'shape': shape, 'active': active}},
                         actions=[op('doorHandle', option=option), op('doorDestination')]))
    for x, y, plane in itertools.product([-1, 0, 1, 2], [-1, 0, 1, 2], [0, 3]):
        rows.append(dict(location=[3207 + x, 3217 + y, plane], objects={'base': {'name': 'cupboard'}},
                         actions=[op('doorHandle', option='open')]))
    for shape in [-2147483648, -1, 0, 1, 2, 3, 4, 9, 10, 22, 2147483647]:
        rows.append(dict(objects={'base': {'shape': shape}}, actions=[op('doorDestination')]))
    for id, permitted, props in itertools.product([-2147483648, 25341, 2147483647], [False, True], [None, 0, 1]):
        for name in ['Door', 'drawers', 'trap door']:
            rows.append(dict(permitted=permitted, properties=props,
                             objects={'base': {'id': id, 'name': name}},
                             actions=[op('doorHandle', option='open'), op('doorHandle', option='close')]))
    for player in [None, 0]:
        for object in [None, 'base']:
            for name in ['Door', 'cupboard', 'trapdoor', 'Stone']:
                rows.append(dict(player=player, objects={'base': {'name': name}},
                                 actions=[op('doorHandle', object=object, option='open'),
                                          op('doorDestination', object=object)]))
    for event in ['register', 'playerLocation', 'audio', 'animate', 'transform', 'replace',
                  'permitted', 'message', 'properties', 'teleport', 'door', 'doorDestination']:
        for mutation in [op('fail'), op('set', location=[3207,3217,2], properties=1, permitted=False),
                         op('set', objects={'base': {'id': 2147483647, 'active': False,
                                                    'name': 'Stone', 'location': [3200,3200,3]}}),
                         op('doorHandle', option='shut', object='other')]:
            rows.append(dict(objects={'base': {'name': 'Wardrobe'}},
                hooks=[dict(event=event, actions=[mutation])],
                actions=[op('register'), op('doorHandle', option='open'),
                         op('set', objects={'base': {'name': 'trapdoor'}}), op('doorHandle', option='open'),
                         op('set', permitted=False), op('doorHandle', option='open'),
                         op('set', objects={'base': {'shape': 0, 'name': 'Door'}}),
                         op('doorDestination'), op('doorHandle', option='open')]))
    rows.append(dict(actions=[op('register'), op('register')]))
    return rows


def generate(check):
    lock = json.loads(read(ROOT / 'upstream.lock.json'))
    reference = ROOT / 'reference/server'
    if run(['git', '-C', reference, 'rev-parse', 'HEAD']).stdout.decode().strip() != lock['server']['commit']:
        raise RuntimeError('Reference pin differs')
    run(['git', '-C', reference, 'diff', '--quiet', 'HEAD', '--'])
    RUN.mkdir(parents=True, exist_ok=True)
    classes = RUN / 'classes'
    classes.mkdir(exist_ok=True)
    jars = c.artifacts()
    std = next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
    artifact = json.loads(read(ROOT / 'oracle/auth/json.lock.json'))
    jar = RUN / 'json-simple-1.1.1.jar'
    if not jar.exists() or hashlib.sha256(jar.read_bytes()).hexdigest() != artifact['sha256']:
        with urllib.request.urlopen(artifact['url'], timeout=60) as response:
            data = response.read()
        if len(data) != artifact['size'] or hashlib.sha256(data).hexdigest() != artifact['sha256']:
            raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    java, kt = [], []

    def adapter(name, text):
        path = RUN / 'adapters' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')
        (kt if path.suffix == '.kt' else java).append(path)

    body = c.method(read(SERVER / 'core/net/packet/PacketProcessor.kt'),
                    'private fun processSceneryAction(pkt: Packet.SceneryAction)')
    adapter('core/net/packet/PacketProcessor.kt', '''package core.net.packet
import core.game.node.entity.player.Player
import core.game.node.scenery.Scenery
import core.game.world.map.Location
import core.game.world.map.RegionManager
import core.game.interaction.InteractPlugin
import core.game.interaction.InteractionListeners
import core.game.interaction.PluginInteractionManager
import core.game.interaction.IntType
import core.game.interaction.Option
import core.net.packet.`in`.Packet
object PacketProcessor {
@JvmStatic fun handle(pkt:Packet.SceneryAction){processSceneryAction(pkt)}
fun sendClearMinimap(player:Player){oracle.Support.emit("minimap")}
''' + body + '\n}')
    source = read(SERVER / 'core/game/world/map/Location.kt')
    adapter('core/game/world/map/Location.kt', '''package core.game.world.map
import kotlin.math.abs
import core.game.node.Node
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;
''' + next(line.strip() for line in source.splitlines() if 'override fun toString()' in line) +
            '\n' + c.method(source, 'override fun equals(other: Any?)') + '\n' +
            c.method(source, 'fun isNextTo(node: Node)') + '\n' +
            next(line.strip() for line in source.splitlines() if 'fun transform(diffX:' in line) +
            '\n@JvmInline ' + c.method(source, 'value class LocationHash(') + '}')
    adapter('core/net/packet/in/Packet.java', '''package core.net.packet.in;
public class Packet{public static class SceneryAction{public core.game.node.entity.player.Player player;public int x,y,id,optIndex;}}''')
    adapter('core/game/node/entity/player/Player.java', '''package core.game.node.entity.player;
import core.game.world.map.Location;public class Player extends core.game.node.Node{public Location location;public Scripts scripts=new Scripts();
public Location getLocation(){oracle.Support.emit("playerLocation");return location;}public Scripts getScripts(){return scripts;}
public oracle.Support.Properties getProperties(){oracle.Support.emit("properties");return oracle.Support.properties==null?null:oracle.Support.props[oracle.Support.properties];}
public Dispatch getPacketDispatch(){return new Dispatch();}public static class Dispatch{public void sendMessage(String s){oracle.Support.emit("message",s);}}
public void debug(String s){oracle.Support.emit("debug",s);}public static class Scripts{public void reset(){oracle.Support.emit("scripts");}}}''')
    adapter('core/game/node/scenery/Scenery.java', '''package core.game.node.scenery;
import core.game.world.map.Location;import core.game.interaction.InteractPlugin;import core.game.node.entity.player.Player;
public class Scenery extends core.game.node.Node{public String key,child,interactionKey,name="Door";public int id,shape=10;public boolean active=true;public Location location;public Direction direction=Direction.NORTH;public Definition definition=new Definition();
public enum Direction{NORTH,EAST,SOUTH,WEST,NORTH_EAST,SOUTH_EAST,SOUTH_WEST,NORTH_WEST;}
public Scenery(){}public Scenery(int id,Location l){oracle.Support.construct(this,id,l);}
public int getId(){return id;}public boolean isActive(){return active;}public Location getLocation(){return location;}public Direction getDirection(){return direction;}
public String getName(){return name;}public int getType(){return shape;}public Scenery asScenery(){return this;}
public Scenery transform(int id){oracle.Support.emit("transform",key,id);return oracle.Support.objects.get("other");}
public Scenery getChild(Player p){oracle.Support.emit("child",key);return child==null?null:oracle.Support.objects.get(child);}
public InteractPlugin getInteraction(){oracle.Support.emit("interaction",key);return interactionKey==null?null:oracle.Support.interactions.get(interactionKey);}
public Definition getDefinition(){return definition;}
public static class Definition{public int configFileId=-1,configId=-1;public Varbit getConfigFile(){if(configFileId==-1)return null;oracle.Support.emit("varbit",configFileId);return oracle.Support.varbit==null?null:new Varbit(oracle.Support.varbit);}public int getConfigId(){return configId;}}
public static class Varbit{int id;public Varbit(int i){id=i;}public int getId(){return id;}}
}''')
    adapter('core/game/world/map/RegionManager.java', '''package core.game.world.map;
import core.game.node.scenery.Scenery;public class RegionManager{public static Scenery getObject(int x,int y,int z,int id,int shape){oracle.Support.emit("lookup",x,y,z,id,shape);return oracle.Support.lookup==null?null:oracle.Support.objects.get(oracle.Support.lookup);}
public static boolean isTeleportPermitted(Location l){oracle.Support.emit("permitted",oracle.Support.coord(l));return oracle.Support.permitted;}}''')
    adapter('core/game/interaction/Option.java', '''package core.game.interaction;public class Option{
public static final Option NULL=new Option("NULL",-1,"NULL");public final String name,key;public final int index;
public Option(String n,int i,String k){name=n;index=i;key=k;}public String getName(){return name;}public int getIndex(){return index;}}''')
    adapter('core/game/interaction/InteractPlugin.java', '''package core.game.interaction;
import core.game.node.entity.player.Player;import core.game.node.scenery.Scenery;public class InteractPlugin{
public String key;public Option[]options;public int generation;public Option get(int i){oracle.Support.emit("option",key,i);return options[i];}
public void handle(Player p,Option o){oracle.Support.emit("handle",key,o.key,o.name,o.index);}
public static void handleInvalidInteraction(Player p,Scenery s,Option o){oracle.Support.emit("invalid",s==null?null:s.key,o.key);}}''')
    adapter('core/game/interaction/IntType.java', 'package core.game.interaction;public enum IntType{SCENERY;}')
    adapter('core/game/interaction/InteractionListeners.java', '''package core.game.interaction;
import core.game.node.entity.player.Player;import core.game.node.scenery.Scenery;public class InteractionListeners{public static boolean run(int id,IntType type,String name,Player p,Scenery s){oracle.Support.emit("listeners",id,name,s.key);return oracle.Support.listeners;}}''')
    adapter('core/game/interaction/PluginInteractionManager.java', '''package core.game.interaction;
import core.game.node.entity.player.Player;import core.game.node.scenery.Scenery;public class PluginInteractionManager{public static boolean handle(Player p,Scenery s){oracle.Support.emit("plugin",s.key);return oracle.Support.plugin;}}''')
    path = SERVER / 'content/global/handlers/scenery/DoorManagingPlugin.java'
    read(path)
    java.append(path)
    adapter('core/game/node/Node.java', '''package core.game.node;
public class Node{public core.game.world.map.Location getLocation(){return null;}public core.game.node.scenery.Scenery asScenery(){return (core.game.node.scenery.Scenery)this;}}''')
    adapter('core/plugin/Plugin.java', 'package core.plugin;public interface Plugin<T>{Plugin<T> newInstance(T arg)throws Throwable;}')
    adapter('core/plugin/Initializable.java', 'package core.plugin;public @interface Initializable{}')
    adapter('core/game/interaction/OptionHandler.java', '''package core.game.interaction;
public abstract class OptionHandler implements core.plugin.Plugin<Object>{
public abstract boolean handle(core.game.node.entity.player.Player p,core.game.node.Node n,String o);
public core.game.world.map.Location getDestination(core.game.node.Node a,core.game.node.Node b){return null;}}''')
    adapter('core/cache/def/impl/SceneryDefinition.java', '''package core.cache.def.impl;
public class SceneryDefinition{public static void setOptionHandler(String s,Object h){oracle.Support.register(s,h);}}''')
    adapter('core/game/global/action/DoorActionHandler.java', '''package core.game.global.action;
import core.game.node.entity.player.Player;import core.game.node.scenery.Scenery;import core.game.world.map.Location;
public class DoorActionHandler{public static Location getDestination(Player p,Scenery s){oracle.Support.emit("doorDestination",s.key);return oracle.Support.destination;}}''')
    adapter('core/game/node/scenery/SceneryBuilder.java', '''package core.game.node.scenery;
public class SceneryBuilder{public static void replace(Scenery s,Scenery n){oracle.Support.emit("replace",s.key,n.key,null);}
public static void replace(Scenery s,Scenery n,int ticks){oracle.Support.emit("replace",s.key,n.key,ticks);}}''')
    adapter('core/api/ContentAPIKt.java', '''package core.api;
import core.game.node.entity.player.Player;import core.game.node.scenery.Scenery;
public class ContentAPIKt{public static void playAudio(Player p,int id){oracle.Support.emit("audio",id);}
public static void animate(Player p,int id,boolean forced){oracle.Support.emit("animate",id,forced);}
public static void handleDoor(Player p,Scenery s){oracle.Support.emit("door",s.key);}}''')
    adapter('org/rs09/consts/Sounds.java', '''package org.rs09.consts;public class Sounds{
public static final int DRAWER_OPEN_64=64,WARDROBE_OPEN_96=96,CUPBOARD_OPEN_58=58,DRAWER_CLOSE_63=63,WARDROBE_CLOSE_95=95,CUPBOARD_CLOSE_57=57;}''')
    # Record the complete class sources behind the deliberately explicit services.
    for path in ['core/game/node/scenery/Scenery.java', 'core/game/interaction/InteractPlugin.java',
                 'core/game/interaction/Option.java', 'core/game/interaction/InteractionListeners.kt',
                 'core/game/interaction/PluginInteractionManager.java',
                 'core/cache/def/impl/SceneryDefinition.java']:
        read(SERVER / path)
    read(HERE / 'Support.java')
    java.append(HERE / 'Support.java')
    cp = os.pathsep.join(map(str, [classes, std, jar]))
    run(['java', '-Xmx512m', '-cp', os.pathsep.join(map(str, jars)), 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',
         '-no-stdlib', '-no-reflect', '-jvm-target', '11', '-classpath', os.pathsep.join(map(str, [std, jar])),
         '-d', classes, *kt, *java])
    run(['javac', '--release', '11', '-encoding', 'UTF-8', '-cp', cp, '-d', classes, *java])
    cases = requests()
    input_file = RUN / 'input.jsonl'
    input_file.write_text(''.join(json.dumps(row) + '\n' for row in cases), encoding='utf-8')
    exe = c.JAVA_HOME / 'bin' / ('java.exe' if os.name == 'nt' else 'java') if c.JAVA_HOME else 'java'
    with input_file.open('rb') as stdin:
        result = subprocess.run(list(map(str, [exe, '-Dfile.encoding=UTF-8', '-cp', cp, 'oracle.Support'])), cwd=ROOT,
                                stdin=stdin, capture_output=True, timeout=120)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace'))
    rows = [json.loads(line) for line in result.stdout.splitlines() if line]
    assert len(rows) == len(cases)
    assert all({k: v for k, v in row.items() if k != 'expected'} == case
               for row, case in zip(rows, cases)), 'Oracle transport changed a scenario'
    data = ''.join(json.dumps(row, sort_keys=True, separators=(',', ':')) + '\n' for row in rows).encode()
    read(pathlib.Path(__file__))
    read(ROOT / 'oracle/login/generate.py')
    provenance = dict(format=1, upstream=lock, sources=dict(sorted(c.SOURCES.items())),
                      caseCount=len(rows), fixtureSha256=hashlib.sha256(data).hexdigest(),
                      sourceHashAlgorithm='sha256, CR removed',
                      scope='Verbatim complete PacketProcessor.processSceneryAction, unchanged complete DoorManagingPlugin, and original LocationHash/equals/isNextTo/transform/toString. Mutable recording object lookup/constructor, morph, retained interaction/option, varbit, scripts, listeners, plugin, invalid/minimap, door/destination, transforms/replacement and player APIs. JVM default locale is explicitly ROOT. Covers lever exceptions, cupboards/drawers/wardrobes/trapdoors, registration, short circuits, late reads, retained receivers, reentrancy and failure prefixes. Real option initialization, listener/plugin registries, approach/arrival, locale selection, door-rule/world service composition, collision, scene fanout and live delivery remain separate.')
    folder = ROOT / 'fixtures/scenery-action'
    if check:
        if (folder / 'oracle.jsonl').read_bytes() != data or json.loads((folder / 'provenance.json').read_text(encoding='utf-8')) != provenance:
            raise RuntimeError('Scenery action fixtures/provenance differ')
        print(f'{len(rows)} original scenery-action scenarios reproduced.')
    else:
        folder.mkdir(parents=True, exist_ok=True)
        (folder / 'oracle.jsonl').write_bytes(data)
        (folder / 'provenance.json').write_text(json.dumps(provenance, sort_keys=True, indent=2) + '\n', encoding='utf-8')
        print(f'Generated {len(rows)} original scenery-action scenarios.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--java-home', type=pathlib.Path)
    args = parser.parse_args()
    c.JAVA_HOME = args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None)
    generate(args.check)
