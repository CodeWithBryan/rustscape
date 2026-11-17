"""Original player renderer and normal rebuild with controlled spatial/mask services."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/player-render-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(kind,**kw):return dict(op=kind,**kw)
def requests():
 out=[];rng=random.Random(530)
 def add(nodes=None,actions=None,**kw):out.append(dict(nodes=nodes or{'self':dict(index=1,location=[3207,3217,0]),'other':dict(index=2,location=[3208,3218,0])},actions=actions or[op('render')],**kw))
 for walk in [-1,0,1,7,8,15]:
  for run in [-1,0,7,8]:
   for required in [False,True]:add({'self':dict(index=1,location=[3207,3217,0],walk=walk,run=run,required=required)})
 for teleport in [False,True]:
  for scene in [False,True]:
   for required in [False,True]:add({'self':dict(index=1,location=[3207,3217,3],teleport=teleport,scene=scene,required=required)},lastScene=[3199,3216,0])
 for dx,dy,z in [(15,0,0),(0,15,0),(15,1,0),(11,10,0),(11,11,0),(0,0,1),(-15,0,0),(-16,0,0)]:
  for teleport in [False,True]:add({'self':dict(index=1,location=[3207,3217,0]),'other':dict(index=2,location=[3207+dx,3217+dy,z],teleport=teleport)},locals=['other'],candidates=['other'])
 for required in [False,True]:
  for synced in [False,True]:
   for stamp in [0,1,-1,2**63-1]:add({'self':dict(index=1,location=[3207,3217,0]),'other':dict(index=2,location=[3208,3218,0],required=required,synced=synced,stamp=stamp)},candidates=['self','other','other'],actions=[op('render'),op('render'),op('set',node='other',fields={'teleport':True}),op('render')])
 for count in [0,1,8,9,10,11,254,255,256]:
  nodes={'self':dict(index=1,location=[3207,3217,0])};nodes.update({f'p{i:03}':dict(index=i+2,location=[3207+i%3,3217+i%5,0],stamp=i+1)for i in range(count)})
  candidates=[k for k in nodes if k!='self'];add(nodes,candidates=candidates,locals=candidates[:count]if count>=254 else[],actions=[op('render'),op('render')])
 for event in ['masks','candidates','send','keys']:
  for action in [op('fail'),op('set',node='self',fields={'location':[3220,3220,2]}),op('clear'),op('push',node='other'),op('erase',index=0),op('replaceList'),op('set',node='other',fields={'stamp':99,'active':False})]:
   add({'self':dict(index=1,location=[3207,3217,0],required=True),'other':dict(index=2,location=[3208,3218,0],required=True)},locals=['other'],candidates=['self','other'],hooks=[dict(event=event,actions=[action])],actions=[op('render'),op('rebuild'),op('render')])
 for x,y in [(-32768,-32768),(-65,-1),(-1,-1),(0,0),(7,7),(3207,3217),(3200,3200),(32767,32767)]:
  for z in range(4):add({'self':dict(index=1,location=[x,y,z])},actions=[op('rebuild'),op('render')])
 for index in [-1,0,1999,2000,2047]:add({'self':dict(index=1,location=[3207,3217,0]),'other':dict(index=index,location=[3208,3218,0],stamp=1)},candidates=['other'])
 for size in [0,2,256]:
  for count in [-1,0,1,255,256]:add(metadata=True,actions=[op('maskArray',size=size),op('infoSet',count=count,prepared=True,first=True),op('registerMask',node='other'),op('registerMask',node=None),op('information'),op('set',node='self',fields={'location':None}),op('information')])
 for _ in range(30):
  nodes={'self':dict(index=1,location=[3207,3217,0])};nodes.update({f'p{i}':dict(index=i+2,location=[3207+rng.randrange(-16,17),3217+rng.randrange(-16,17),rng.randrange(2)],walk=rng.randrange(-1,9),run=rng.choice([-1,0,1,7]),required=bool(rng.randrange(2)),synced=bool(rng.randrange(2)),active=rng.randrange(5)!=0,invisible=rng.randrange(5)==0,stamp=rng.randrange(3))for i in range(12)})
  names=list(nodes)[1:];add(nodes,locals=names[:6],candidates=names+names[:3],actions=[op('render'),op('render')])
 return out
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'))
 for part in ['server','client']:
  p=ROOT/'reference'/part
  if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
  run(['git','-C',p,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 paths=['core/game/world/update/PlayerRenderer.kt','core/game/node/entity/player/info/RenderInfo.java','core/net/packet/out/UpdateSceneGraph.java','core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java','core/net/packet/OutgoingPacket.java','core/net/packet/Context.java','core/net/packet/context/SceneGraphContext.java','core/net/event/GameWriteEvent.java','core/net/IoWriteEvent.java']
 for name in paths:p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
 for name in ['core/cache/crypto/ISAACCipher.java','core/cache/misc/buffer/ByteBufferUtils.java']:adapter(name,read(ROOT/'oracle/scenery/adapters'/name))
 adapter('core/net/IoSession.java','''package core.net;import java.nio.ByteBuffer;import core.net.packet.IoBuffer;public class IoSession{public void queue(ByteBuffer b){byte[]v=new byte[b.remaining()];b.get(v);oracle.Support.sent(v);}public void write(IoBuffer b){oracle.Support.emit("send",b.opcode());new core.net.event.GameWriteEvent(this,b).write(this,b);}public IoSession getIsaacPair(){return this;}public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}public void disconnect(){throw new UnsupportedOperationException();}}''')
 adapter('core/ServerConstants.java','package core;public class ServerConstants{public static int MAX_PLAYERS=2000;}')
 adapter('core/game/node/entity/Entity.java','package core.game.node.entity;public class Entity{}')
 adapter('core/game/node/entity/npc/NPC.java','package core.game.node.entity.npc;public class NPC extends core.game.node.entity.Entity{}')
 adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;import core.game.world.map.Location;import core.game.node.entity.player.info.RenderInfo;public class Player extends core.game.node.entity.Entity{public String name;public int index;public Location location;public boolean active=true,invisible;public RenderInfo renderInfo=new RenderInfo(this);public Flags playerFlags=new Flags();public Props properties=new Props();public Walking walkingQueue=new Walking();public Masks updateMasks=new Masks(this);public Direction direction=new Direction();public Player(String s){name=s;}public Location getLocation(){return location;}public boolean isActive(){return active;}public boolean isInvisible(){return invisible;}public int getIndex(){return index;}public RenderInfo getRenderInfo(){return renderInfo;}public Flags getPlayerFlags(){return playerFlags;}public Props getProperties(){return properties;}public Walking getWalkingQueue(){return walkingQueue;}public Masks getUpdateMasks(){return updateMasks;}public Direction getDirection(){return direction;}public Player getDetails(){return this;}public core.net.IoSession getSession(){return oracle.Support.session;}public static class Direction{public int value;public int getOrdinal(){return value;}}public static class Flags{public boolean updateSceneGraph;public Location lastSceneGraph;public boolean isUpdateSceneGraph(){return updateSceneGraph;}public Location getLastSceneGraph(){return lastSceneGraph;}public void setLastSceneGraph(Location l){lastSceneGraph=l;}}public static class Props{public boolean teleporting;public boolean isTeleporting(){return teleporting;}}public static class Walking{public int walkDir=-1,runDir=-1;public int getWalkDir(){return walkDir;}public int getRunDir(){return runDir;}}public static class Masks{Player owner;public boolean required,synced;public long appearanceStamp;Masks(Player p){owner=p;}public long getAppearanceStamp(){return appearanceStamp;}public boolean isUpdateRequired(){return required;}public boolean hasSynced(){return synced;}public void write(Player p,Player e,core.net.packet.IoBuffer b){oracle.Support.masks(p,e,b,false,false);}public void writeSynced(Player p,Player e,core.net.packet.IoBuffer b,boolean a){oracle.Support.masks(p,e,b,a,true);}}}''')
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static java.util.List<core.game.node.entity.player.Player>getLocalPlayers(Location l,int d){return oracle.Support.candidates(l,d);}}')
 adapter('core/game/system/config/XteaParser.java','package core.game.system.config;public class XteaParser{public static XteaParser Companion=new XteaParser();public int[]getRegionXTEA(int id){return oracle.Support.keys(id);}}')
 location=read(SERVER/'core/game/world/map/Location.kt')
 adapter('core/game/world/map/Location.kt','''package core.game.world.map
import kotlin.math.sqrt
class Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val regionX:Int get()=x shr 3;val regionY:Int get()=y shr 3;val sceneX:Int get()=x-((regionX-6) shl 3);val sceneY:Int get()=y-((regionY-6) shl 3);fun getSceneX(l:Location):Int=x-((l.regionX-6) shl 3);fun getSceneY(l:Location):Int=y-((l.regionY-6) shl 3);@JvmOverloads fun withinDistance(other:Location,dist:Int=15)=other.z==z&&getDistance(this,other)<=dist;companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z);'''+c.method(location,'fun getDistance(first: Location, second: Location)')+'}\n@JvmInline '+c.method(location,'value class LocationHash(')+'}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=180)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged PlayerRenderer, RenderInfo, UpdateSceneGraph, IoBuffer and original game writer. Explicit mutable player/mask/spatial/XTEA services and original packed location/distance observations. Compares frame bytes, local-list aliases/order/mutations, appearance stamps, callback/failure prefixes and normal rebuild publication. Concrete mask contents, region spatial enumeration, full entity/profile/startup/tick/socket integration remain separate.')
 folder=ROOT/'fixtures/player-render'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Player render fixtures/provenance differ')
  print(f'{len(out)} original player-render/rebuild scenarios reproduced.')
 else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original player-render/rebuild scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
