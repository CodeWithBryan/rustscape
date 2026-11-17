"""Joined original MovementPulse, destination dispatch, SmartPathfinder and WalkingQueue."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/player-walk-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(operation,**kw):return dict(op=operation,**kw)
def requests():
 out=[]
 def op(operation,**kw):return dict(op=operation,**kw)
 def add(actions,**kw):out.append(dict(actions=actions,**kw))
 for bfs in [False,True]:
  for running in [False,True]:
   for kind in ['world','interact','minimap']:
    for distance in [0,1,7,25,26,40]:add([op('request',kind=kind,x=3207+distance,y=3216,running=running)]+[op('tick')for _ in range(30)],bfs=bfs)
 for bfs in [False,True]:
  for running in [False,True]:
   add([op('request',x=3214,y=3218,running=running)]+[op('tick')for _ in range(20)],bfs=bfs,flags=[[3210,y,0x12c01ff]for y in range(3213,3220)])
 for phase in ['locked','dialogue','face','faceLocation','resetScripts','weak','normal','clip','timer','zone']:
  add([op('request',x=3214,y=3218,running=True),op('tick'),op('tick'),op('request',x=3209,y=3217),op('tick')],hooks=[dict(event=phase,actions=[op('fail')])])
 for running in [False,True]:
  for n in [1,2,8]:add([op('request',x=3210+i,y=3216,running=running)for i in range(n)]+[op('tick')for _ in range(16)])
 for frozen in [False,True]:
  for energy in [0,0.9,1,20,100]:add([op('request',x=3214,y=3218,running=True)]+[op('tick')for _ in range(16)],frozen=frozen,energy=energy)
 for locked in [False,True]:add([op('request'),op('tick')],locked=locked)
 for phase in ['clip','timer','zone']:
  add([op('request',x=3214,y=3218),op('tick'),op('tick'),op('tick')],hooks=[dict(event=phase,actions=[op('request',x=3209,y=3217,running=True)])])
 for zone in [[False],[True,False],[True,True,False]]:add([op('request',x=3214,y=3218,running=True)]+[op('tick')for _ in range(12)],zoneAnswers=zone)
 add([op('request',x=3214,y=3218,running=True),op('tick'),op('mutatePulse',mover=None,force=False,checks=None),op('arrival')])
 for shape in [0,2,9,10,11,22]:
  for rotation in range(4):
   for size in [[1,1],[2,3]]:
    add([op('approach')]+[op('tick')for _ in range(10)],scenery=dict(shape=shape,rotation=rotation,size=size))
 for walking in [0,1,2,4,8,15]:
  add([op('approach')]+[op('tick')for _ in range(8)],scenery=dict(shape=10,walking=walking),handler=None)
 for destination in [None,[3208,3216,0]]:
  add([op('request',kind='interact',running=True),op('approach')]+[op('tick')for _ in range(8)],scenery={},optionDestination=destination)
 for change in [op('scenerySet',location=[3213,3218,0]),op('scenerySet',active=False),
                op('optionReplace',name='Search',index=4,handler=2),op('optionHandler',handler=2)]:
  add([op('approach'),op('tick'),change]+[op('tick')for _ in range(9)],scenery={})
 add([op('approach'),op('tick'),op('mutatePulse'),op('arrival')],scenery={})
 for phase in ['destination','permitted','doorDestination','interactionArrival','clip','zone']:
  for change in [op('fail'),op('scenerySet',location=[3212,3218,0]),op('optionHandler',handler=2)]:
   add([op('approach')]+[op('tick')for _ in range(8)],scenery=dict(shape=0 if phase=='doorDestination' else 10),handler=None if phase=='permitted'else 1,hooks=[dict(event=phase,actions=[change])])
 return out

def generate(check, extension=None):
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
 for name in ['core/game/interaction/MovementPulse.java','core/game/interaction/DestinationFlag.java','core/game/system/task/Pulse.java','core/game/world/map/Direction.java','core/game/world/map/Point.java','core/game/world/map/path/Path.java','core/game/world/map/path/ClipMaskSupplier.java','core/api/utils/Vector.kt','core/game/node/entity/impl/WalkingQueue.java','core/game/world/map/path/SmartPathfinder.kt','core/game/node/entity/impl/PulseManager.java','core/game/node/entity/impl/PulseType.java','core/game/world/PulseRunner.kt']:
  p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
 node=read(SERVER/'core/game/node/Node.java')
 adapter('core/game/node/Node.java','''package core.game.node;import core.game.world.map.*;import core.api.utils.Vector;public class Node{public String name;public Location location;public int size=1;public Direction direction=Direction.SOUTH;public boolean active=true;public Location getLocation(){return location;}public int size(){return size;}public Direction getDirection(){return direction;}public boolean isActive(){oracle.Support.emit("active",name);return active;}'''+c.method(node,'public Location getCenterLocation()')+c.method(node,'public Vector getMathematicalCenter()')+'}')
 location=read(SERVER/'core/game/world/map/Location.kt')
 methods='\n'.join(next(line.strip()for line in location.splitlines()if sig in line)for sig in ['fun transform(diffX: Int, diffY: Int, z: Int)','fun transform(dir: Direction, steps: Int','fun transform(vector: Vector)'])
 adapter('core/game/world/map/Location.kt','''package core.game.world.map
import core.api.utils.Vector
import kotlin.math.*
class Location(x:Int,y:Int,z:Int=0):core.game.node.Node(){var hash=LocationHash(x,y,(z%4+4)%4);var x:Int get()=hash.x;set(v){hash=LocationHash(v,y,z)};var y:Int get()=hash.y;set(v){hash=LocationHash(x,v,z)};var z:Int get()=hash.z;set(v){hash=LocationHash(x,y,(v%4+4)%4)};override fun getLocation()=this;fun getRegion():oracle.Support.Region?=oracle.Support.region(this);fun getDistance(l:Location)=getDistance(this,l);override fun equals(other:Any?)=other is Location&&hash==other.hash;override fun hashCode()=hash.hash;
'''+methods+'''
companion object{@JvmStatic @JvmOverloads fun create(x:Int,y:Int,z:Int=0)=Location(x,y,z);@JvmStatic fun getDelta(a:Location,b:Location)=Location(b.x-a.x,b.y-a.y,b.z-a.z);'''+c.method(location,'fun getDistance(first: Location, second: Location)')+'};@JvmInline '+c.method(location,'value class LocationHash(')+'}')
 adapter('core/game/node/entity/npc/NPC.java','package core.game.node.entity.npc;public class NPC extends core.game.node.entity.Entity{public NPCBehavior behavior=new NPCBehavior();public boolean never;public Integer finder;public boolean isNeverWalks(){return never;}}')
 adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;public class Scenery extends core.game.node.Node{public int type=10,rotation;public Definition definition=new Definition();public int getType(){return type;}public int getRotation(){return rotation;}public Definition getDefinition(){return definition;}public static class Definition{public int sizeX=1,sizeY=1,walkingFlag;public int getWalkingFlag(){return walkingFlag;}}}')
 source=read(SERVER/'core/game/world/map/path/Pathfinder.java');names=['canDecorationInteract','canDoorInteract','isStandingIn','canInteract','canInteractSized']
 adapter('core/game/global/action/DoorActionHandler.java','package core.game.global.action;public class DoorActionHandler{public static core.game.world.map.Location getDestination(core.game.node.entity.Entity m,core.game.node.scenery.Scenery s){return oracle.Support.door(m,s);}}')
 for name,typ in [('OptionHandler','core.game.node.entity.Entity'),('UseWithHandler','core.game.node.entity.player.Player')]:
  service='option'if name=='OptionHandler'else'use'
  adapter('core/game/interaction/'+name+'.java','package core.game.interaction;public class '+name+'{int token;public '+name+'(int t){token=t;}public core.game.world.map.Location getDestination('+typ+' m,core.game.node.Node d){return oracle.Support.destination("'+service+'",token,m,d);}}')
 api=read(SERVER/'core/api/ContentAPI.kt');read(SERVER/'core/ServerConstants.kt')
 adapter('core/api/ContentAPI.kt','''package core.api
import core.api.utils.Vector
import core.game.node.entity.Entity
import core.game.node.entity.npc.NPC
import core.game.world.map.Location
import core.ServerConstants
import kotlin.math.min
'''+c.method(api,'fun truncateLoc (')+'''
fun clearHintIcon(p:core.game.node.entity.player.Player){oracle.Support.emit("hintClear",p.name)}
fun registerHintIcon(p:core.game.node.entity.player.Player,l:Location?,kind:Int){oracle.Support.emit("hintAdd",p.name,l?.let{listOf(it.x,it.y,it.z)},kind)}
''')
 adapter('core/ServerConstants.java','package core;public class ServerConstants{public static int MAX_PATHFIND_DISTANCE=25;}')
 adapter('core/net/packet/context/PlayerContext.java','package core.net.packet.context;public class PlayerContext{public core.game.node.entity.player.Player player;public PlayerContext(core.game.node.entity.player.Player p){player=p;}}')
 adapter('core/net/packet/PacketRepository.java','package core.net.packet;public class PacketRepository{public static void send(Class c,core.net.packet.context.PlayerContext p){oracle.Support.emit("minimap",p.player.name);}}')
 for path,decl in [('core/net/packet/out/ClearMinimapFlag.java','package core.net.packet.out;public class ClearMinimapFlag{}'),('core/tools/SystemLogger.java','package core.tools;public class SystemLogger{}'),('content/region/wilderness/handlers/revenants/RevenantNPC.java','package content.region.wilderness.handlers.revenants;public class RevenantNPC{}')]:adapter(path,decl)
 from adapters import compose
 compose(adapter,read,c,SERVER,RUN/'adapters')
 # Retained Option instances use the complete unchanged original value class;
 # its uncalled default-handler lookups remain explicit definition adapters.
 p=SERVER/'core/game/interaction/Option.java';read(p);java.append(p)
 for name in ['ItemDefinition','NPCDefinition','SceneryDefinition']:
  adapter('core/cache/def/impl/'+name+'.java','package core.cache.def.impl;public class '+name+'{public static core.game.interaction.OptionHandler getOptionHandler(int id,String name){return null;}}')
 adapter('core/game/node/item/Item.java','package core.game.node.item;public class Item extends core.game.node.Node{public Item(int id){}}')
 read(ROOT/'oracle/navigation/adapters.py')
 java=list(dict.fromkeys(java));kt=list(dict.fromkeys(kt))
 java=[p for p in java if not p.as_posix().endswith('adapters/core/game/node/entity/impl/WalkingQueue.java') and not p.as_posix().endswith('adapters/core/game/world/GameWorld.java')]
 read(HERE/'adapters.py')
 read(HERE/'Support.java');java.append(HERE/'Support.java')
 if extension is not None:extension.install(adapter,read,c,SERVER,RUN/'adapters',java,kt)
 java=list(dict.fromkeys(java));kt=list(dict.fromkeys(kt))
 cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests()if extension is None else extension.requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 main='oracle.Support'if extension is None else extension.MAIN
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx2g','-Dfile.encoding=UTF-8','-cp',cp,main])),cwd=ROOT,stdin=f,capture_output=True,timeout=240)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases)
 assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases)),'Oracle transport changed a scenario'
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Joined verbatim walk handler/arrival and Entity.tick with unchanged MovementPulse, DestinationFlag, PulseManager, PulseType, PulseRunner, SmartPathfinder, WalkingQueue, Path, Point, Direction, Vector and complete Option plus original dispatch/reach/truncate methods. Native scenery target views/approaches share canonical movement queues, retained option/target captures and the normal scheduler. Explicit player/content/door destination/teleport and recording interaction-arrival services compare mutation/reentrancy/failure order. The recording approach class identity oracle.Support$1 is normalized to its original InteractPlugin$3 telemetry identity. Original InteractPlugin arrival bodies, option default lookup, full world/content and live scenery handling remain separate.')
 if extension is not None:provenance['scope']=extension.SCOPE
 folder=ROOT/'fixtures'/('player-walk'if extension is None else extension.FIXTURE)
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Player walk fixtures/provenance differ')
  print(f'{len(rows)} joined original player-walk scenarios reproduced.')
 else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} joined original player-walk scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
