"""Joined original MovementPulse, destination dispatch, Smart/DumbPathfinder and WalkingQueue."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/navigation-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(name,**kw):return dict(op=name,**kw)
def requests():
 out=[];rng=random.Random(530)
 def add(actions,nodes=None,**kw):out.append(dict(nodes=nodes or dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='location',location=[3214,3218,0])),actions=actions,**kw))
 for bfs in [False,True]:
  for force in [False,True]:
   for target in ['location','player','npc','scenery']:
    for size in [1,2,3]:
     add([op('new',finder=0,force=force)]+[a for _ in range(12) for a in [op('update'),op('walk')]],nodes=dict(mover=dict(type='player',location=[3207,3216,0],size=size),target=dict(type=target,location=[3214,3218,0],kind=10,sx=2,sy=3,rotation=1)),bfs=bfs)
 for bfs in [False,True]:
  for force in [False,True]:
   for energy in [0,0.9,1,25,100]:
    add([op('new',finder=0,force=force)]+[a for _ in range(12) for a in [op('update'),op('walk')]],nodes=dict(mover=dict(type='player',location=[3207,3216,0],energy=energy),target=dict(type='location',location=[3214,3218,0])),bfs=bfs,flags=[[3210,y,0x12c01ff]for y in range(3213,3220)])
 for callback in ['zone','timer','face','add','clip']:
  add([op('new',finder=0),op('update'),op('walk'),op('update'),op('walk')],hooks=[dict(event=callback,actions=[op('fail')])])
 for callback in ['zone','timer','face','add','clip']:
  add([op('new',finder=0),op('update'),op('walk'),op('update'),op('walk')],hooks=[dict(event=callback,actions=[op('node',node='target',location=[3213,3214,0])])])
 for callback in ['zone','timer','clip']:
  for action in [op('node',node='mover',location=[3205,3220,1]),op('new',finder=0)]:
   add([op('new',finder=0,force=True),op('update'),op('walk'),op('update'),op('walk')],hooks=[dict(event=callback,actions=[action])])
 for zone in [[False],[True,False],[True,True,False]]:
  add([op('new',finder=0,force=True)]+[a for _ in range(8)for a in[op('update'),op('walk')]],zoneAnswers=zone)
 for frozen in [False,True]:
  for hamstrung in [False,True]:add([op('new',finder=0,force=True)]+[a for _ in range(12)for a in[op('update'),op('walk')]],frozen=frozen,hamstrung=hamstrung)
 for bfs in [False,True]:
  for combat in [False,True]:
   for running in [False,True]:
    add([op('walkPlan',node='target',x=3220,y=3220,running=running),op('new',finder=0,constructor='flag',flag='follow')]+[a for _ in range(16)for a in[op('update'),op('walk',node='target'),op('walk')]],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='npc',location=[3214,3218,0],combat=combat,victim='mover')),bfs=bfs,pulseResult=False)
  add([op('new',finder=0),op('update'),op('walk'),op('update')],bfs=bfs,clip=-1)
  add([op('new',finder=0)]+[a for _ in range(14)for a in[op('update'),op('walk')]],bfs=bfs,flags=[[3214,3218,0x12c01ff]])
 for distance in [0,1,14,25,26,40]:add([op('new',finder=0)]+[a for _ in range(30)for a in[op('update'),op('walk')]],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='location',location=[3207+distance,3216,0])))
 for _ in range(24):
  add([op('new',finder=0,force=bool(rng.randrange(2)))]+[a for _ in range(14)for a in[op('update'),op('walk')]],nodes=dict(mover=dict(type=rng.choice(['player','npc']),location=[3207,3216,0],size=rng.randrange(1,4),finder=0),target=dict(type=rng.choice(['location','npc','scenery']),location=[3207+rng.randrange(-8,9),3216+rng.randrange(-8,9),0],kind=rng.choice([0,2,9,10,11,22]),rotation=rng.randrange(4),sx=2,sy=3)),bfs=bool(rng.randrange(2)))
 for size in [1,2,3,4]:
  for target in ['location','npc','scenery']:
   for blocked in [False,True]:
    add([op('new',finder=1)]+[a for _ in range(16)for a in[op('update'),op('walk')]],nodes=dict(mover=dict(type='npc',location=[3207,3216,0],size=size,finder=1,behavior=True),target=dict(type=target,location=[3214,3218,0],kind=10,rotation=1,sx=2,sy=3)),flags=[[3210,y,0x12c01ff]for y in range(3213,3220)]if blocked else[])
 for kind in [0,1,2,4,6,7,9,10,11,22]:
  add([op('new',finder=1)]+[a for _ in range(10)for a in[op('update'),op('walk')]],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='scenery',location=[3210,3218,0],kind=kind,rotation=2,sx=2,sy=3)))
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
 for name in ['core/game/interaction/MovementPulse.java','core/game/interaction/DestinationFlag.java','core/game/system/task/Pulse.java','core/game/world/map/Direction.java','core/game/world/map/Point.java','core/game/world/map/path/Path.java','core/game/world/map/path/ClipMaskSupplier.java','core/api/utils/Vector.kt','core/game/node/entity/impl/WalkingQueue.java','core/game/world/map/path/SmartPathfinder.kt']:
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
 compose(adapter,read,c,SERVER)
 java=list(dict.fromkeys(java));kt=list(dict.fromkeys(kt))
 java=[p for p in java if not p.as_posix().endswith('adapters/core/game/node/entity/impl/WalkingQueue.java') and not p.as_posix().endswith('adapters/core/game/world/GameWorld.java')]
 read(HERE/'adapters.py')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx2g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=240)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Joined unchanged MovementPulse, DestinationFlag, Pulse, WalkingQueue, SmartPathfinder, Path, Point, Direction and Vector with verbatim Pathfinder overloads/reach and truncateLoc. Explicit node/entity/settings/script/region/zone/timer/content/packet services. Actual native queues, destination dispatch and smart searches replace isolated adapters; compares per-command fields, locations/aliases, queue consumption, energy, callbacks and failures. Original complete world scheduling, live walk handling, content and entity membership remain separate.')
 folder=ROOT/'fixtures/navigation'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Navigation fixtures/provenance differ')
  print(f'{len(rows)} joined original navigation scenarios reproduced.')
 else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} joined original navigation scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
