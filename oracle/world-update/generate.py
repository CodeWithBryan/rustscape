"""Original packet buffers, initializing repository and update sequence replay."""
import argparse, hashlib, importlib.util, json, os, pathlib, subprocess, urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2]; HERE=pathlib.Path(__file__).resolve().parent
RUN=ROOT/'runs/world-update-oracle'; SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py')
c=importlib.util.module_from_spec(spec); spec.loader.exec_module(c); read,run=c.read,c.run

def requests():
    cases=[]
    def add(actions,**kw): cases.append(dict(actions=actions,**kw))
    def a(op,**kw): return dict(op=op,**kw)
    base=[a('start'),a('run'),a('end')]
    add(base); add([a('run'),a('end'),a('terminate')])
    for p in [[],[1],[1,2],[1,1,2]]:
        for n in [[],[3],[3,4]]: add(base*2,players=p,npcs=n,lobby=[])
    for op in ['add','remove']:
        for node in [None,0,1,2,6]:
            add([a(op,list='players',node=node),a(op,list='players',node=node),a('sync'),a('sync'),*base])
    add([a('add',list='players',node=6),a('remove',list='players',node=6),a('add',list='players',node=6),a('sync')])
    for phase in ['active','renderable','minimap','npcTick','playerTick','playerUpdate','playerReset','npcReset','clock','npcMetric','playerMetric','renderMetric','chunks','ground']:
        for at in [1,2]:
            add([a('add',list='players',node=6),*base,a('sync'),*base],failures={phase:at})
    mutations=[a('add',list='players',node=6),a('remove',list='players',node=1),a('clear',list='players'),a('removeAt',list='players',index=0),a('set',list='players',index=1,node=6),a('insert',list='players',index=0,node=6),a('sync'),a('repo',kind='npcs',list='other'),a('captured',kind='npcs',list='other'),a('captured',kind='players',list=None)]
    for phase in ['minimap','npcTick','playerTick','playerUpdate','playerReset','npcReset','active','renderable','chunks']:
        for mutation in mutations:
            add([a('add',list='players',node=6),*base,*base],hooks=[dict(event=phase,actions=[mutation])])
    # Iterator hasNext, replacement without modCount, and captured repository aliases.
    for phase, target in [('minimap','lobby'),('npcTick','npcs'),('playerTick','players'),('playerUpdate','players'),('playerReset','players'),('npcReset','npcs')]:
        for op in ['clear','removeAt','set','insert']:
            add(base*2,hooks=[dict(event=phase,actions=[a(op,list=target,index=0,node=6)])])
    add([a('start'),a('repo',kind='npcs',list='other'),a('run'),a('end'),*base])
    for clock in [[0,1,2,3,4,5],[9223372036854775807,-9223372036854775808],[-9223372036854775808,9223372036854775807],[0,2147483648]]: add(base,clock=clock)
    for nulls in [('players',[None]),('npcs',[None]),('lobby',[None])]: add(base,**{nulls[0]:nulls[1]})
    for count in [0,1,3,20]:
        add([*[a('enqueue',id=i,fail=i%2==1)for i in range(count)],a('process'),a('process')])
    for hook in [[a('enqueue',id=8)], [a('no')], [a('enqueue',id=8),a('process')], [a('enqueue',id=8),a('no'),a('enqueue',id=9),a('process')]]:
        add([a('enqueue',id=1),a('enqueue',id=2),a('enqueue',id=3,fail=True),a('process'),a('process'),a('enqueue',id=10),a('process')],hooks=[dict(event='packet',actions=hook)])
    for pos in range(4):
        events=[a('enqueue',id=i)for i in range(3)];events.insert(pos,a('no'))
        add([*events,a('process'),a('process')],hooks=[dict(event='packet',actions=[a('enqueue',id=8)])])
    for phase in ['packet','log']:
        for at in [1,2,3]:
            add([a('enqueue',id=1,fail=True),a('enqueue',id=2,fail=True),a('enqueue',id=3),a('process'),a('process'),a('enqueue',id=4),a('process'),a('process')],failures={phase:at})
    add([a('enqueue',id=1,fail=True),a('enqueue',id=2,fail=True),a('process'),a('process')],hooks=[dict(event='log',actions=[a('enqueue',id=8),a('process')])])
    joined=[a('enqueue',id=1),a('enqueue',id=2,fail=True),a('handle'),a('handle',skip=True),a('handle')]
    add(joined,hooks=[dict(event='packet',actions=[a('add',list='players',node=6),a('enqueue',id=8)])])
    for phase in ['packetTime','disconnects','pulses','listeners','npcTick','playerTick','playerUpdate','playerReset','npcReset','active','renderable','chunks','ground','pulseWorld','managers','flush','exception']:
        for at in [1,2]:
            add([a('add',list='players',node=6),*joined],failures={phase:at})
    for phase in ['packet','disconnects','pulses','listeners','playerTick','playerUpdate','playerReset','chunks','flush']:
        add(joined,hooks=[dict(event=phase,actions=[a('add',list='players',node=6),a('remove',list='players',node=1),a('enqueue',id=8)])])
    for failures in [dict(playerTick=1,flush=1),dict(playerTick=1,exception=1),dict(playerTick=1,exception=1,flush=1),dict(flush=1,exception=1)]:add(joined,failures=failures)
    for ticks in [-2147483648,-1,2147483647]:add(joined,ticks=ticks)
    for phase in ['npcTick','npcReset']:
        for op in ['clear','removeAt','set','insert']:
            add(base*2,hooks=[dict(event=phase,actions=[a(op,list='npcs',index=0,node=7)])])
    for name,node in [('npcs',7),('lobby',6)]:
        add([a('add',list=name,node=node),a('add',list=name,node=node),a('remove',list=name,node=node),*base])
    for phase in ['npcTick','playerTick']:
        add([a('handle'),a('handle')],hooks=[dict(event=phase,actions=[a('set',list='npcs',index=0,node=6)])])
    return cases

def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json')); ref=ROOT/'reference/server'
    if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']: raise RuntimeError('Reference pin differs')
    run(['git','-C',ref,'diff','--quiet','HEAD','--']); RUN.mkdir(parents=True,exist_ok=True)
    classes=RUN/'classes'; classes.mkdir(exist_ok=True); jars=c.artifacts()
    std=next(p for p in jars if p.name.startswith('kotlin-stdlib-')); jar=RUN/'json-simple-1.1.1.jar'
    artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
    if not jar.exists() or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    java=[]; kt=[]
    def adapter(name,text):
        p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8')
        (kt if p.suffix=='.kt'else java).append(p)
    for name in ['core/game/world/repository/InitializationEntry.java','core/game/world/repository/InitializingNodeList.java','core/game/world/update/UpdateSequence.kt']:
        read(SERVER/name);(kt if name.endswith('.kt')else java).append(SERVER/name)
    repository=read(SERVER/'core/game/world/repository/Repository.kt')
    assert 'public val RENDERABLE_NPCS: MutableList<NPC> = CopyOnWriteArrayList()'in repository
    assert 'get() = RENDERABLE_NPCS'in repository
    source=read(SERVER/'core/net/packet/PacketProcessor.kt')
    adapter('core/net/packet/PacketProcessor.kt','''package core.net.packet
import core.net.packet.`in`.Packet
import core.api.log
import core.tools.Log
import java.util.*
import java.io.*
object PacketProcessor {
private var queue=LinkedList<Packet>();private var processingQueue=LinkedList<Packet>();private val queueLock=Any()
'''+c.method(source,'@JvmStatic fun enqueue(pkt: Packet)')+'\n'+c.method(source,'@JvmStatic fun processQueue()')+'''
private fun process(pkt:Packet){oracle.Support.process(pkt)}
@JvmStatic fun reset(){queue.clear();processingQueue.clear()}
@JvmStatic fun snapshot()=listOf(queue.map{it.id},processingQueue.map{it.id})
}''')
    worker=read(SERVER/'core/worker/MajorUpdateWorker.kt')
    adapter('core/worker/MajorUpdateWorker.kt','''package core.worker
import core.game.world.GameWorld
import core.game.world.repository.Repository
import core.net.packet.PacketProcessor
import core.net.packet.PacketWriteQueue
import core.plugin.Managers
import core.integrations.grafana.Grafana
object System{@JvmStatic fun currentTimeMillis()=oracle.Support.clock()}
class MajorUpdateWorker(val sequence:core.game.world.update.UpdateSequence){
'''+c.method(worker,'fun handleTickActions(skipPulseUpdate: Boolean = false)')+'}')
    adapter('core/net/packet/in/Packet.java','''package core.net.packet.in;public class Packet{public int id;public boolean fail;public Packet(int i,boolean f){id=i;fail=f;}public static class NoProcess extends Packet{public NoProcess(){super(-999,false);}}}''')
    node=read(SERVER/'core/game/node/Node.java')
    adapter('core/game/node/Node.java','''package core.game.node;public class Node{public int id;public boolean active,renderable;public Node(int i){id=i;}'''+c.method(node,'public void setActive(boolean active)')+c.method(node,'public void setRenderable(boolean renderable)')+'}')
    adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;public class Player extends core.game.node.Node{public Player(int i){super(i);}public void tick(){oracle.Support.emit("playerTick",id);}public void update(){oracle.Support.emit("playerUpdate",id);}public void reset(){oracle.Support.emit("playerReset",id);}@Override public void setActive(boolean b){super.setActive(b);oracle.Support.emit("active",id,b);}@Override public void setRenderable(boolean b){super.setRenderable(b);oracle.Support.emit("renderable",id,b);}}''')
    adapter('core/game/node/entity/npc/NPC.java','''package core.game.node.entity.npc;public class NPC extends core.game.node.Node{public NPC(int i){super(i);}public void tick(){oracle.Support.emit("npcTick",id);}public void reset(){oracle.Support.emit("npcReset",id);}}''')
    adapter('core/game/world/repository/Repository.java','''package core.game.world.repository;import java.util.*;import core.game.node.entity.player.Player;import core.game.node.entity.npc.NPC;public class Repository{public static List<Player> lobbyPlayers;public static List<NPC> renderableNpcs;public static oracle.Support.Phase disconnectionQueue=new oracle.Support.Phase("disconnects");}''')
    adapter('core/game/world/GameWorld.java','''package core.game.world;import java.util.*;public class GameWorld{public static oracle.Support.Phase Pulser=new oracle.Support.Phase("pulses");public static List<oracle.Support.Phase>tickListeners=Arrays.asList(new oracle.Support.Phase("listeners"));public static void pulse(){oracle.Support.emit("pulseWorld");oracle.Support.ticks++;}}''')
    adapter('core/net/packet/PacketWriteQueue.java','package core.net.packet;public class PacketWriteQueue{public static void flush(){oracle.Support.emit("flush");}}')
    adapter('core/plugin/Managers.java','package core.plugin;public class Managers{public static void tick(){oracle.Support.emit("managers");}}')
    adapter('core/game/world/update/System.java','package core.game.world.update;public class System{public static long currentTimeMillis(){return oracle.Support.clock();}}')
    adapter('core/game/world/update/ChunkUpdateTracker.java','package core.game.world.update;public class ChunkUpdateTracker{public static void resetAll(){oracle.Support.emit("chunks");}}')
    adapter('core/game/node/item/GroundItemManager.java','package core.game.node.item;public class GroundItemManager{public static void pulse(){oracle.Support.emit("ground");}}')
    adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{}')
    adapter('core/net/packet/context/PlayerContext.java','package core.net.packet.context;public class PlayerContext{public core.game.node.entity.player.Player player;public PlayerContext(core.game.node.entity.player.Player p){player=p;}}')
    adapter('core/net/packet/out/ClearMinimapFlag.java','package core.net.packet.out;public class ClearMinimapFlag{}')
    adapter('core/net/packet/PacketRepository.java','package core.net.packet;public class PacketRepository{public static void send(Class c,core.net.packet.context.PlayerContext p){if(p.player==null)throw new NullPointerException();oracle.Support.emit("minimap",p.player.id);}}')
    adapter('core/integrations/grafana/Grafana.kt','''package core.integrations.grafana
object Grafana{var packetProcessTime=0;set(v){field=v;oracle.Support.emit("packetTime",v)};var npcTickTime=0;set(v){field=v;oracle.Support.emit("npcMetric",v)};var playerTickTime=0;set(v){field=v;oracle.Support.emit("playerMetric",v)};var playerRenderTime=0;set(v){field=v;oracle.Support.emit("renderMetric",v)}}''')
    adapter('core/tools/Log.java','package core.tools;public enum Log{ERR}')
    adapter('core/api/Log.kt','''package core.api
fun log(c:Class<*>,level:core.tools.Log,message:String){oracle.Support.emit("log",message)}''')
    read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java])
    run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8')
    exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=60)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases)
    data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode()
    read(pathlib.Path(__file__)); read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged InitializingNodeList, InitializationEntry and UpdateSequence with verbatim PacketProcessor.enqueue/processQueue and MajorUpdateWorker.handleTickActions, including joined tick replays. Concrete nodes, packet dispatch, exception text, clock, telemetry, packet output, chunks and ground items are explicit observable services. Repository-backed ArrayList lobby/InitializingNodeList players and CopyOnWriteArrayList NPC iteration are explicit. Compares deferred duplicates/removals, immediate ordinary-list changes, live queues, aliasing, iterator snapshots/modification checks, sequence captures/phases, partial failures, wrapping clocks/ticks, nested packet processing, global NoProcess and cumulative error logs. Full Player/NPC initialization/content and packet handlers remain separate.')
    folder=ROOT/'fixtures/world-update'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('World-update fixtures/provenance differ')
        print(f'{len(rows)} original packet/repository/update scenarios reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original packet/repository/update scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args()
    c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
