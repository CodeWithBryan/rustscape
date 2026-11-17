"""Unchanged SmartPathfinder and original reach methods with sparse clipping."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/pathfinding-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
 cases=[];rng=random.Random(530)
 for op in ['door','decoration','standing','rectangle','sized']:
  for mover in [0,1,2,3]:
   for dx,dy in [(0,0),(-1,0),(1,0),(0,-1),(0,1),(-2,-2),(2,2),(0,10),(2,-10)]:
    for kind in [0,2,6,7,8,9]:
     for rotation in range(4):cases.append(dict(op=op,from_=[3207,3216,0],to=[3207+dx,3216+dy],mover=[mover,mover],kind=kind,rotation=rotation,size=[2,3],default=rng.choice([0,0,0x80,0x20,0x2,0x8,0x12c01ff]),worldDefault=rng.choice([0,0,0x2,0x80]),walkingFlag=rng.randrange(16)))
 for row in cases:row['from']=row.pop('from_')
 for op in ['door','decoration','standing','rectangle','sized']:
  for _ in range(100):cases.append(dict(op=op,**{'from':[rng.choice([-2147483648,-1,0,2147483647]),rng.choice([-2147483648,0,2147483647]),3]},to=[rng.choice([-2147483648,0,2147483647]),rng.choice([-2147483648,0,2147483647])],mover=[rng.randrange(-2,5),rng.randrange(-2,5)],size=[rng.randrange(-2,5),rng.randrange(-2,5)],kind=rng.randrange(-1,11),rotation=rng.randrange(-1,5),default=-1,worldDefault=-1))
 # Bounded sized helpers keep oracle iterations finite even for wrapping inputs.
 for op in ['door','decoration','rectangle','sized']:
  for fail in [1,2]:cases.append(dict(op=op,**{'from':[3206,3216,0]},to=[3207,3216],mover=[1,1],size=[1,1],kind=9,failAt=fail))
 for bfs in [False,True]:
  for mover in [1,2,3,4]:
   for near in [False,True]:
    for dx,dy in [(0,0),(1,0),(-1,0),(0,1),(0,-1),(4,3),(-4,-3),(24,0),(26,0),(36,10),(50,0)]:cases.append(dict(op='find',end=[3207+dx,3216+dy,0],mover=[mover,mover],size=[0,0],bfs=bfs,near=near))
 for bfs in [False,True]:
  for mover in [1,2,3]:
   for near in [False,True]:
    for kind in [0,1,3,7,8,9,10]:
     flags=[[3209,y,0x12c01ff]for y in range(3212,3220)]
     cases.append(dict(op='find',end=[3212,3218,0],mover=[mover,mover],size=[2,2]if kind==0 else[0,0],kind=kind,rotation=1,bfs=bfs,near=near,flags=flags))
 for bfs in [False,True]:
  for mover in [1,2,3]:
   for at in [1,2,10,100]:cases.append(dict(op='find',end=[3213,3219,0],mover=[mover,mover],size=[0,0],bfs=bfs,near=True,failAt=at))
 for mutate in ['mutateStart','mutateEnd']:
  for bfs in [False,True]:cases.append(dict(op='find',end=[3212,3218,0],mover=[1,1],size=[0,0],bfs=bfs,near=True,mutateAt=5,**{mutate:[3200,3220,1]}))
 for start,end in [([-1,-1,3],[1,1,3]),([16383,16383,2],[16380,16380,2]),([-16384,-16384,0],[-16380,-16380,0]),([3207,3216,3],[3237,3216,3])]:cases.append(dict(op='find',start=start,end=end,mover=[1,1],size=[0,0],near=True))
 for k in ['start','end']:cases.append(dict(op='find',**{k:None}))
 cases.append(dict(op='find',nullSupplier=True))
 for _ in range(40):
  flags=[[3207+rng.randrange(-8,9),3216+rng.randrange(-8,9),rng.choice([0x100,0x80,0x20,0x2,0x8,0x12c01ff])]for _ in range(40)]
  cases.append(dict(op='find',end=[3207+rng.randrange(-8,9),3216+rng.randrange(-8,9),0],mover=[rng.randrange(1,4),1],size=[0,0],bfs=bool(rng.randrange(2)),near=bool(rng.randrange(2)),flags=flags))
 for bfs in [False,True]:
  for mover in [0,1,2,3,5]:
   for near in [False,True]:cases.append(dict(op='find',end=[3212,3218,0],mover=[mover,mover],size=[0,0],bfs=bfs,near=near,default=-1))
 for bfs in [False,True]:
  for near in [False,True]:cases.append(dict(op='find',end=[3212,3218,0],mover=[1,1],size=[0,0],bfs=bfs,near=near,flags=[[3212,3218,0x12c01ff]]))
 for _ in range(100):
  start=[3200+rng.randrange(8),3216+rng.randrange(8),rng.randrange(4)]
  flags=[[start[0]+rng.randrange(-15,16),start[1]+rng.randrange(-15,16),rng.choice([0x100,0x80,0x20,0x2,0x8,0x12c01ff])]for _ in range(80)]
  cases.append(dict(op='find',start=start,end=[start[0]+rng.randrange(-12,13),start[1]+rng.randrange(-12,13),start[2]],mover=[rng.randrange(0,6),1],size=[rng.randrange(4),rng.randrange(4)],kind=rng.randrange(12),rotation=rng.randrange(4),walkingFlag=rng.randrange(16),bfs=bool(rng.randrange(2)),near=bool(rng.randrange(2)),flags=flags,worldFlags=flags[::-1]))
 return cases
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
 for name in ['core/game/world/map/path/SmartPathfinder.kt','core/game/world/map/Direction.java','core/game/world/map/Point.java','core/game/world/map/path/ClipMaskSupplier.java','core/game/world/map/path/Path.java','core/api/utils/Vector.kt']:
  p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
 source=read(SERVER/'core/game/world/map/path/Pathfinder.java');methods=['canDecorationInteract','canDoorInteract','isStandingIn','canInteract','canInteractSized','flagForDirection'];constants=re.findall(r'public static final int [^;]+;',source)
 adapter('core/game/world/map/path/Pathfinder.java','package core.game.world.map.path;import core.game.world.map.*;public abstract class Pathfinder{'+''.join(constants)+''.join(c.method(source,'public static '+('int 'if name=='flagForDirection'else'boolean ')+name+'(')for name in methods)+'public abstract Path find(Location a,int s,Location b,int sx,int sy,int r,int t,int f,boolean n,ClipMaskSupplier c);}')
 location=read(SERVER/'core/game/world/map/Location.kt')
 adapter('core/game/world/map/Location.kt','''package core.game.world.map
import kotlin.math.floor
import core.api.utils.Vector
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);var x:Int get()=hash.x;set(v){hash=LocationHash(v,y,z)};var y:Int get()=hash.y;set(v){hash=LocationHash(x,v,z)};var z:Int get()=hash.z;set(v){hash=LocationHash(x,y,(v%4+4)%4)};val regionX:Int get()=x shr 3;val regionY:Int get()=y shr 3;val sceneX:Int get()=x-((regionX-6)shl 3);val sceneY:Int get()=y-((regionY-6)shl 3);fun getSceneX(l:Location)=x-((l.regionX-6)shl 3);fun getSceneY(l:Location)=y-((l.regionY-6)shl 3);override fun equals(other:Any?)=other is Location&&hash==other.hash;override fun hashCode()=hash.hash;'''+next(line.strip()for line in location.splitlines()if'fun transform(vector: Vector)'in line)+''';companion object{@JvmStatic @JvmOverloads fun create(x:Int,y:Int,z:Int=0)=Location(x,y,z)};@JvmInline '''+c.method(location,'value class LocationHash(')+'}')
 adapter('core/game/world/GameWorld.kt','package core.game.world;object GameWorld{var settings:Settings?=Settings()};class Settings{var smartpathfinder_bfs=false}')
 adapter('core/ServerConstants.java','package core;public class ServerConstants{public static int MAX_PATHFIND_DISTANCE=25;}');read(SERVER/'core/ServerConstants.kt')
 adapter('core/tools/Log.java','package core.tools;public enum Log{FINE;}')
 adapter('core/api/Logging.kt','package core.api;fun log(c:Class<*>,l:core.tools.Log,s:String){}')
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static int getClippingFlag(int z,int x,int y){return oracle.Support.clip(z,x,y,true);}}')
 adapter('core/game/node/entity/Entity.java','package core.game.node.entity;public class Entity{public Locks getLocks(){return new Locks();}public Walking getWalkingQueue(){return new Walking();}public static class Locks{public boolean isMovementLocked(){return false;}}public static class Walking{public void reset(){}public void addPath(int x,int y){}}}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx2g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=300)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');read(ROOT/'tools/port_reach.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged SmartPathfinder with original Point/Direction/Vector and five verbatim Pathfinder reach predicates plus direction flags. Explicit mutable packed Location, sparse supplier/global clipping, settings, logging and Path.walk entity adapters. Compares points/results/errors, all ordered clipping calls via SHA256 and prefix/suffix, full via/cost/ring-array SHA256 and cursor state. Full Pathfinder Node dispatch, Path.walk, MovementPulse, server phase/scene/live movement remain separate. Malformed native sized-reach iteration bounded to65536.')
 folder=ROOT/'fixtures/pathfinding'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Pathfinding fixtures/provenance differ')
  print(f'{len(rows)} original pathfinding/reach scenarios reproduced.')
 else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original pathfinding/reach scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
