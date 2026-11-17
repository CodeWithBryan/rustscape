"""Pinned complete NPCRenderer with original buffers/writer and observable services."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/npc-render-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
    cases=[];rng=random.Random(53032)
    def a(op,**kw):return dict(op=op,**kw)
    def add(npcs=None,actions=None,**kw):cases.append(dict(npcs=npcs if npcs is not None else {'a':dict(index=1,id=1,location=[3208,3218,0])},actions=actions or[a('render')],**kw))
    add({})
    add({'a':dict(index=7,id=1,location=[3208,3218,0],hidden=True),'b':dict(index=7,id=4,location=[3209,3218,0])},locals=['a'],candidates=['b'],actions=[a('render'),a('render')])
    for walk in [-1,0,1,7,8,15]:
        for running in [-1,0,1,7,8]:
            for required in [False,True]:add({'a':dict(index=1,id=1,location=[3208,3218,0],walk=walk,run=running,required=required)},locals=['a'])
    for dx,dy,z in [(15,15,0),(16,0,0),(0,-16,0),(-15,-15,0),(11,11,0),(0,0,1)]:
        for hidden in [False,True]:
            for teleport in [False,True]:add({'a':dict(index=1,id=1,location=[3207+dx,3217+dy,z],hidden=hidden,teleport=teleport,aggressive=True)},locals=['a'],candidates=['a'])
    for count in [0,1,9,10,11,254,255,256]:
        npcs={f'n{i:03}':dict(index=i+1,id=i%100,location=[3207+i%3,3217+i%5,0],required=i%3==0)for i in range(count)}
        names=list(npcs);add(npcs,candidates=names+names[:2],locals=names if count>=254 else[],actions=[a('render'),a('render')])
    for index in [-1,0,32766,32767,32768]:
        for id in [-1,0,1,16383,16384]:add({'a':dict(index=index,id=id,location=[3208,3218,0],direction=7,required=True)},candidates=['a'])
    for direction in [-1,0,1,7,8,15]:add({'a':dict(index=1,id=4,location=[3208,3218,0],direction=direction,teleport=True)},candidates=['a'])
    for phase in ['hidden','masks','candidates','removeTolerance','addTolerance','send']:
        for mutation in [a('fail'),a('clear'),a('push',node='b'),a('erase',index=0),a('replaceList'),a('set',node='self',fields=dict(index=9,location=[3220,3220,2])),a('set',node='a',fields=dict(location=[3220,3220,2],id=81,aggressive=False)),a('set',node='b',fields=dict(hidden=True,required=True))]:
            add({'a':dict(index=1,id=1,location=[3223,3217,0],required=True,aggressive=True),'b':dict(index=2,id=4,location=[3208,3218,0],required=True,aggressive=True)},locals=['a','b'],candidates=['a','b'],hooks=[dict(event=phase,actions=[mutation])],actions=[a('render'),a('render')])
    for local in [[],['a'],[None],['a','a']]:
        for candidate in [[],['a','a'],[None],['a',None]]:add(locals=local,candidates=candidate)
    for _ in range(32):
        npcs={f'n{i}':dict(index=i+1,id=rng.randrange(100),location=[3207+rng.randrange(-18,19),3217+rng.randrange(-18,19),rng.randrange(2)],walk=rng.randrange(-1,8),run=rng.choice([-1,1,7]),required=bool(rng.randrange(2)),hidden=rng.randrange(5)==0,aggressive=bool(rng.randrange(2)))for i in range(16)}
        names=list(npcs);add(npcs,locals=names[:8],candidates=names+names[:3],actions=[a('render'),a('render')])
    return cases
def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
    if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
    run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    java=[];kt=[]
    def adapter(name,text):
        p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
    for name in ['core/game/world/update/NPCRenderer.kt','core/game/world/map/MapDistance.java','core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java','core/net/event/GameWriteEvent.java','core/net/IoWriteEvent.java']:
        p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
    for name in ['core/cache/crypto/ISAACCipher.java','core/cache/misc/buffer/ByteBufferUtils.java']:adapter(name,read(ROOT/'oracle/scenery/adapters'/name))
    adapter('core/net/IoSession.java','''package core.net;import java.nio.ByteBuffer;import core.net.packet.IoBuffer;public class IoSession{public void queue(ByteBuffer b){byte[]v=new byte[b.remaining()];b.get(v);oracle.Support.sent(v);}public void write(IoBuffer b){oracle.Support.emit("send",b.opcode());new core.net.event.GameWriteEvent(this,b).write(this,b);}public IoSession getIsaacPair(){return this;}public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}public void disconnect(){throw new UnsupportedOperationException();}}''')
    adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;import core.game.world.map.Location;import core.game.node.entity.npc.NPC;import java.util.*;public class Player{public int index=1;public Location location=Location.create(3207,3217,0);public Info renderInfo=new Info();public int getIndex(){return index;}public Location getLocation(){return location;}public Info getRenderInfo(){return renderInfo;}public core.net.IoSession getSession(){return oracle.Support.session;}public static class Info{public List<NPC> localNPCs=new LinkedList<>();public List<NPC>getLocalNPCs(){return localNPCs;}}}''')
    adapter('core/game/node/entity/npc/NPC.java','''package core.game.node.entity.npc;import core.game.world.map.Location;import core.game.node.entity.player.Player;import core.net.packet.IoBuffer;import java.util.*;public class NPC{public String name;public int index,id;public Location location;public boolean hidden;public Props properties=new Props();public Walking walkingQueue=new Walking();public Masks updateMasks=new Masks(this);public Direction direction=new Direction();public Aggression aggressiveHandler;public NPC(String name){this.name=name;}public int getIndex(){return index;}public int getId(){return id;}public Location getLocation(){return location;}public Props getProperties(){return properties;}public Walking getWalkingQueue(){return walkingQueue;}public Masks getUpdateMasks(){return updateMasks;}public Direction getDirection(){return direction;}public Aggression getAggressiveHandler(){return aggressiveHandler;}public boolean isHidden(Player p){oracle.Support.emit("hidden",name);return hidden;}public static class Props{public boolean teleporting;public boolean isTeleporting(){return teleporting;}}public static class Walking{public int walkDir=-1,runDir=-1;public int getWalkDir(){return walkDir;}public int getRunDir(){return runDir;}}public static class Direction{public int ordinal;public int getOrdinal(){return ordinal;}}public static class Masks{NPC owner;public boolean required;Masks(NPC n){owner=n;}public boolean isUpdateRequired(){return required;}public void write(Player p,NPC n,IoBuffer b){oracle.Support.masks(n,b,false);}public void writeSynced(Player p,NPC n,IoBuffer b,boolean a){oracle.Support.masks(n,b,true);}}public static class Aggression{public NPC owner;public Map<Integer,Integer>playerTolerance=new HashMap<Integer,Integer>(){@Override public Integer put(Integer k,Integer v){oracle.Support.emit("addTolerance",owner.name,k,v);return super.put(k,v);}};public Aggression(NPC n){owner=n;}public void removeTolerance(int index){oracle.Support.emit("removeTolerance",owner.name,index);playerTolerance.remove(index);}}}''')
    adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static java.util.List<core.game.node.entity.npc.NPC>getLocalNPCs(Location l,int d){return oracle.Support.candidates(l,d);}}')
    adapter('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{public static int ticks=100;public static int getTicks(){return ticks;}}')
    location=read(SERVER/'core/game/world/map/Location.kt')
    maxnorm=next(line.strip()for line in location.splitlines()if'fun withinMaxnormDistance('in line)
    adapter('core/game/world/map/Location.kt','''package core.game.world.map
import kotlin.math.max
import kotlin.math.abs
class Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;
'''+maxnorm+'''
companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)};@JvmInline '''+c.method(location,'value class LocationHash(')+'}')
    read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Xmx1g','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=60)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    rows=[json.loads(l)for l in result.stdout.splitlines()if l];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged NPCRenderer, IoBuffer and original game writer; original packed location and maxnorm method. Explicit NPC visibility/mask/aggression/spatial/player/clock services. Compares original frame bytes, live LinkedList membership and retained aliases, direction/movement fields, near/plane checks, 255 capacity, duplicate candidates, tolerance ordering and reentrant callback/failure prefixes. Concrete masks, complete NPC/definition/spawn/AI/combat/region/session integration remain separate.')
    folder=ROOT/'fixtures/npc-render'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('NPC-render fixtures/provenance differ')
        print(f'{len(rows)} original NPC-render scenarios reproduced.')
    else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original NPC-render scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
