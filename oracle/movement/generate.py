"""Complete original MovementPulse/DestinationFlag with explicit world services."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/movement-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(name,**kw):return dict(op=name,**kw)
def requests():
 out=[];rng=random.Random(530)
 def add(actions,nodes=None,**kw):out.append(dict(nodes=nodes or dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='location',location=[3210,3216,0])),actions=actions,**kw))
 for m in ['player','npc','entity']:
  for target in ['location','player','npc','entity','scenery']:
   for force in [False,True]:add([op('new',force=force),op('update'),op('node',location=[3210,3216,0]),op('update'),op('update'),op('stop')],nodes=dict(mover=dict(type=m,location=[3207,3216,0]),target=dict(type=target,location=[3210,3216,0])))
 for constructor in ['option','use','flag','override']:
  for flag in [None,'location','entity','follow','combat','custom']:
   for loc in [None,'@target',[3209,3216,0]]:add([op('new',constructor=constructor,flag=flag),op('path'),op('update')],**{constructor+'Location':loc})
 for flag in ['location','entity','follow','combat','item','object']:
  for size in [1,2,3]:
   for dx,dy in [(-3,0),(3,0),(0,-3),(0,3),(0,0)]:
    add([op('flagDestination',flag=flag),op('closest',flag=flag,suggestion=[3210,3215,0])],nodes=dict(mover=dict(type='player',location=[3210+dx,3216+dy,0],size=size),target=dict(type='scenery',location=[3210,3216,0],size=size,kind=10,sx=2,sy=3,rotation=rng.randrange(4))),clip=rng.choice([0,0,0x80,0x20,0x2,0x8]))
 for kind in [0,1,2,3,4,5,9,10,11,22]:
  for rotation in range(4):
   for walk in [0,7,15]:add([op('flagDestination',flag='object')],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='scenery',location=[3210,3216,0],kind=kind,rotation=rotation,walkFlag=walk,sx=2,sy=3)))
 for m in ['player','npc']:
  for dx,dy in [(0,0),(1,0),(14,0),(25,0),(26,0),(-25,-1)]:
   for plane in [0,3]:add([op('truncate',location=[3207+dx,3216+dy,plane]),op('new'),op('path')],nodes=dict(mover=dict(type=m,location=[3207,3216,plane]),target=dict(type='location',location=[3207+dx,3216+dy,plane])))
 for region in [None,False,True]:add([op('new'),op('update'),op('stop'),op('update')],region=region)
 for fields in [dict(active=False),dict(never=True),dict(behavior=False),dict(finder=2),dict(points=[[3207,3216],[3208,3216]],running=True)]:add([op('new'),op('update'),op('new',finder=2),op('path')],nodes=dict(mover=dict(type='npc',location=[3207,3216,0],**fields),target=dict(type='player',location=[3210,3216,0])))
 for running in [False,True]:
  for moving in [False,True]:
   for combat in [False,True]:add([op('new'),op('update'),op('node',node='target',location=[3211,3216,0]),op('update')],nodes=dict(mover=dict(type='player',location=[3207,3216,0],running=running),target=dict(type='npc',location=[3210,3216,0],points=[[3210,3216],[3211,3216],[3212,3216]],moving=moving,combat=combat,victim='mover')))
 for success in [False,True]:
  for near in [False,True]:add([op('new'),op('path'),op('node',location=[3210,3216,0]),op('update')],paths=[dict(successful=success,near=near,points=[[3208,3216],[3210,3216]])])
 add([op('new'),op('set',target=None),op('update'),op('path')])
 add([op('new'),op('node',location=None),op('update'),op('path')])
 add([op('new'),op('checks',values=['mover',None]),op('set',target=None),op('update')])
 add([op('new'),op('path'),op('pathView'),op('new'),op('path')],paths=[dict(points=[[3208,3216],[3210,3216]])])
 for event in ['region','active','weak','normal','face','destination','find','reset','add','pulse','minimap','message','hintClear','hintAdd','clip','permitted','doorDestination','npcFinder']:
  for hook in [op('fail'),op('set',running=False),op('set',target=None),op('node',node='target',location=[3211,3218,1]),op('pathClear'),op('checks',values=None)]:
   # Do not mutate a nonexistent route from early callbacks.
   if hook['op']=='pathClear'and event not in ['reset','add','face']:continue
   add([op('new',constructor='flag',flag='entity'),op('update'),op('node',location=[3210,3216,0]),op('update'),op('new'),op('path')],nodes=dict(mover=dict(type='player',location=[3207,3216,0],draw=True),target=dict(type='npc',location=[3210,3216,0])),hooks=[dict(event=event,actions=[hook])])
 for at in [1,2]:add([op('new'),op('path'),op('pathView'),op('path')],hooks=[dict(event='add',at=at,actions=[op('pathReplace')])],paths=[dict(points=[[3208,3216],[3209,3216],[3210,3216]])])
 add([op('new'),op('set',last='shared',interactLocation='shared',previousLoc='shared'),op('mutateLocation',which='shared',x=3210,y=3216,z=3),op('update')],locations=dict(shared=[3207,3216,0]))
 for _ in range(40):
  nodes=dict(mover=dict(type=rng.choice(['player','npc']),location=[3207,3216,rng.randrange(4)],size=rng.randrange(1,4)),target=dict(type=rng.choice(['location','npc','scenery']),location=[3207+rng.randrange(-4,5),3216+rng.randrange(-4,5),0],size=rng.randrange(1,4),moving=bool(rng.randrange(2)),points=[[3210,3216],[3211,3217]],rotation=rng.randrange(4)))
  add([op('new',force=bool(rng.randrange(2))),op('path'),op('update'),op('stop'),op('start'),op('update')],nodes=nodes,pulseResult=bool(rng.randrange(2)))
 for event in ['pulse','minimap','face','exception']:
  for at in [1,2]:
   for hook in [op('fail'),op('set',target=None),op('set',running=False),op('new',target='target')]:
    add([op('new'),op('update'),op('update'),op('stop')],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='player',location=[3208,3216,0])),hooks=[dict(event=event,at=at,actions=[hook])])
 for event in ['clip','permitted','reset','add','destination','find']:
  for hook in [op('set',near=True,usingTruncatedPath=True,forceRun=True),op('set',target=None),op('node',node='mover',location=[3205,3220,2]),op('node',node='target',size=3,location=[3212,3218,1]),op('new'),op('pathClear')]:
   add([op('new'),op('path'),op('update')],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='player',location=[3210,3216,0])),hooks=[dict(event=event,actions=[hook])])
 for constructor in ['option','use','flag','override']:
  for typ in ['npc','entity']:
   add([op('new',constructor=constructor,flag='entity'),op('path'),op('update')],nodes=dict(mover=dict(type=typ,location=[3207,3216,0]),target=dict(type='player',location=[3210,3216,0])))
 for rotation in [-33,-1,4,31,32,33]:
  for walk in [-1,0,7,15,2147483647]:add([op('flagDestination',flag='object')],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='scenery',location=[3210,3216,0],kind=10,rotation=rotation,walkFlag=walk,sx=2,sy=3)))
 for answers in [[True,None],[True,False],[False,True],[True,True,False]]:add([op('new'),op('update'),op('update')],regionAnswers=answers)
 for m,d in [(None,'target'),('mover',None),(None,None)]:add([op('new',mover=m,target=d),op('update')])
 for points in [[],[[3210,3216]],[[3210,3216],[3211,3216]]]:
  for victim in [None,'mover']:add([op('new'),op('path'),op('path')],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='npc',location=[3210,3216,0],points=points,combat=True,victim=victim)))
 for event in ['clip','permitted']:
  for hook in [op('node',node='target',size=0),op('node',node='target',rotation=3,sx=3,sy=2),op('node',node='mover',location=[3214,3216,0])]:add([op('flagDestination',flag='object'),op('closest',flag='entity',suggestion=[3210,3215,0])],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='scenery',location=[3210,3216,0],kind=10,rotation=0,sx=2,sy=3)),hooks=[dict(event=event,actions=[hook])])
 for end in [[3210,3216,0],[3240,3216,0]]:add([op('new'),op('set',near=True,usingTruncatedPath=True,interactLocation=[3208,3216,0]),op('path')],nodes=dict(mover=dict(type='player',location=[3207,3216,0]),target=dict(type='npc',location=end)),paths=[dict(null=True)])
 for start,end in [([-16384,-16384,3],[16383,16383,3]),([16383,16383,0],[-16384,-16384,0]),([-1,0,3],[0,-1,3])]:
  add([op('flagDestination',flag='entity'),op('new'),op('path')],nodes=dict(mover=dict(type='player',location=start),target=dict(type='npc',location=end)))
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
 for name in ['core/game/interaction/MovementPulse.java','core/game/interaction/DestinationFlag.java','core/game/system/task/Pulse.java','core/game/world/map/Direction.java','core/game/world/map/Point.java','core/game/world/map/path/Path.java','core/game/world/map/path/ClipMaskSupplier.java','core/api/utils/Vector.kt']:
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
 adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;import core.game.node.Node;import core.game.interaction.MovementPulse;public class Entity extends Node{public MovementPulse currentMovement;public core.game.node.entity.impl.WalkingQueue walking=new core.game.node.entity.impl.WalkingQueue(this);public Props props=new Props();public Scripts scripts=new Scripts(this);public boolean draw;public core.game.node.entity.impl.WalkingQueue getWalkingQueue(){return walking;}public Props getProperties(){return props;}public core.game.node.entity.npc.NPC asNpc(){return(core.game.node.entity.npc.NPC)this;}public void face(Entity e){oracle.Support.emit("face",name,e==null?null:e.name);}public boolean getAttribute(String k,boolean d){return draw;}public Locks getLocks(){return new Locks();}public static class Locks{public boolean isMovementLocked(){return false;}}public static class Props{public Combat combat=new Combat();public Combat getCombatPulse(){return combat;}}public static class Combat{public boolean running;public Entity victim;public boolean isRunning(){return running;}public Entity getVictim(){return victim;}}public static class Scripts{Entity e;Scripts(Entity e){this.e=e;}public void removeWeakScripts(){oracle.Support.emit("weak",e.name);}public void removeNormalScripts(){oracle.Support.emit("normal",e.name);}}}''')
 adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;public class Player extends core.game.node.entity.Entity{public Dispatch getPacketDispatch(){return new Dispatch();}public class Dispatch{public void sendMessage(String s){oracle.Support.emit("message",name,s);}}}''')
 adapter('core/game/node/entity/npc/NPC.java','package core.game.node.entity.npc;public class NPC extends core.game.node.entity.Entity{public NPCBehavior behavior=new NPCBehavior();public boolean never;public Integer finder;public boolean isNeverWalks(){return never;}}')
 adapter('core/game/node/entity/npc/NPCBehavior.java','package core.game.node.entity.npc;public class NPCBehavior{public core.game.world.map.path.Pathfinder getPathfinderOverride(NPC n){return oracle.Support.npcFinder(n);}}')
 adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;public class Scenery extends core.game.node.Node{public int type=10,rotation;public Definition definition=new Definition();public int getType(){return type;}public int getRotation(){return rotation;}public Definition getDefinition(){return definition;}public static class Definition{public int sizeX=1,sizeY=1,walkingFlag;public int getWalkingFlag(){return walkingFlag;}}}')
 adapter('core/game/node/entity/impl/WalkingQueue.java','''package core.game.node.entity.impl;import java.util.*;import core.game.world.map.*;import core.game.node.entity.Entity;public class WalkingQueue{Entity owner;Deque<Point>points=new ArrayDeque<>();public boolean running,moving;public WalkingQueue(Entity e){owner=e;}public Deque<Point>getQueue(){return points;}public boolean hasPath(){return !points.isEmpty();}public boolean isRunning(){return running;}public boolean isMoving(){return moving;}public void reset(){oracle.Support.emit("reset",owner.name,null);seed();}public void reset(boolean r){oracle.Support.emit("reset",owner.name,r);seed();running=r;}private void seed(){Location l=owner.getLocation();points.clear();points.add(new Point(l.getX(),l.getY()));}public void addPath(int x,int y){oracle.Support.emit("add",owner.name,x,y);points.add(new Point(x,y));}}''')
 source=read(SERVER/'core/game/world/map/path/Pathfinder.java');names=['canDecorationInteract','canDoorInteract','isStandingIn','canInteract','canInteractSized']
 adapter('core/game/world/map/path/Pathfinder.java','package core.game.world.map.path;import core.game.node.Node;import core.game.node.entity.Entity;import core.game.world.map.*;public class Pathfinder{public static final Pathfinder SMART=new Pathfinder(0),DUMB=new Pathfinder(1);public int id;public Pathfinder(int id){this.id=id;}public static Path find(Entity m,Node d,boolean near,Pathfinder f){return oracle.Support.find(m,d,near,f);}'+''.join(c.method(source,'public static boolean '+name+'(')for name in names)+'}')
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static int getClippingFlag(int z,int x,int y){return oracle.Support.clip(z,x,y);}public static boolean isTeleportPermitted(Location l){return oracle.Support.permitted(l);}}')
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
 for path,decl in [('core/net/packet/out/ClearMinimapFlag.java','package core.net.packet.out;public class ClearMinimapFlag{}'),('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{}'),('core/tools/SystemLogger.java','package core.tools;public class SystemLogger{}'),('content/region/wilderness/handlers/revenants/RevenantNPC.java','package content.region.wilderness.handlers.revenants;public class RevenantNPC{}')]:adapter(path,decl)
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx2g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=240)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged MovementPulse, DestinationFlag, Pulse, Point, Direction, Path and Vector; verbatim truncateLoc, Node centers and five reach predicates. Explicit mutable node/region/behavior/combat/script, queue/pathfinder, option/use/override, clipping, teleport, door, hint/packet and normalized exception services. Compares pulse fields, aliases, queue/path consumption, callbacks, failures and all destination geometry. Actual WalkingQueue/SmartPathfinder, entity/profile/content/world scheduling and live movement integration remain separate. Malformed native geometry loops are bounded.')
 folder=ROOT/'fixtures/movement'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Movement fixtures/provenance differ')
  print(f'{len(rows)} original movement/destination scenarios reproduced.')
 else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original movement/destination scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
