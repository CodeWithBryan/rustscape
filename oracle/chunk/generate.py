"""Original RegionChunk scenery-storage methods with explicit external adapters."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/chunk-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
def read(p):return c.read(pathlib.Path(p))
def run(args):return c.run(list(map(str,args)))
def op(kind,**kw):return dict(op=kind,**kw)
def requests():
 cases=[];rng=random.Random(530)
 base={k:dict(id=i,location=[3207,3217,0],shape=0,rotation=i%4)for i,k in enumerate(['a','b','c','d','e'],1)};base['same']=dict(base['a']);base['built']=dict(base['a'],constructed=True)
 def add(actions,**kw):cases.append(dict(nodes=base,actions=actions,**kw))
 for node in base:
  for static in [True,False]:
   add([op('active',node=node,value=False),op('renderable',node=node,value=False),op('charge',node=node,value=88),op('static'if static else'add',node=node),op('add',node=node),op('index',id=base[node]['id'],shape=22),op('copy',name='copy'),op('currentBase',chunk='copy',value=[3224,3224,1]),op('rebase',chunk='copy'),op('resetFlags',chunk='copy'),op('rebase')])
 for x,y in [(0,0),(7,7),(-1,0),(0,-1),(8,0),(0,8),(-1,8),(1,-1)]:
  add([op('static',node='a',x=x,y=y),op('index',x=x,y=y),op('copy',name='copy')])
 add([op('static',node=k)for k in ['a','same','a','b','c','d','e']]+[op('index',id=-1,shape=s)for s in [-1,0,1,22]])
 add([op('add',node=k)for k in ['a','same','built','b','c','d','e']]+[op('index',id=i,shape=22)for i in [-2,-1,1,2,99]])
 add([op('failId',value=1),op('static',node='a'),op('failId',value=None),op('static',node='a'),op('add',node='a')])
 add([op('static',node='a'),op('failId',value=1),op('copy',name='copy'),op('failId',value=None),op('copy',name='copy')])
 for r in [-4,-1,0,1,2,3,4,2**31-1]:
  for cr in [-1,0,1,2,3,4]:
   for size in [(1,1),(2,3),(-1,0),(2**31-1,-2**31)]:
    add([op('rotatePosition',x=7,y=-2,sx=size[0],sy=size[1],rotation=r,chunkRotation=cr)])
 add([op('static',node='a'),op('copy',name='copy'),op('cell',node='a',static=True,x=0,y=0,slot=2),op('cell',node='a',x=6,y=3,slot=3),op('currentBase',value=[-16384,16383,5]),op('rebase'),op('copy',name='next')])
 for _ in range(50):
  acts=[]
  for _ in range(15):
   kind=rng.choice(['static','add','cell','index','copy','rebase','currentBase','active','renderable','charge','rotate','location','resetFlags','rotation'])
   value=bool(rng.randrange(2))if kind in ['active','renderable']else[rng.randrange(-100,33000),rng.randrange(-100,33000),rng.randrange(-2,6)]if kind in ['currentBase','location']else rng.choice([-1,0,1,2,7,88])
   acts.append(op(kind,node=rng.choice(list(base)),name='copy',x=rng.randrange(-1,9),y=rng.randrange(-1,9),slot=rng.randrange(-1,5),static=bool(rng.randrange(2)),id=rng.randrange(-1,7),shape=rng.randrange(-1,23),value=value))
  add(acts)
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
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if name.endswith('.kt')else java).append(p)
 for path in ['core/game/node/scenery/Scenery.java','core/game/node/scenery/Constructed.java','core/game/system/task/Pulse.java']:
  p=SERVER/path;read(p);java.append(p)
 for path in ['core/game/node/Node.java','core/game/node/entity/impl/GameAttributes.java','core/game/interaction/InteractPlugin.java','core/game/interaction/DestinationFlag.java','core/cache/def/impl/VarbitDefinition.java','core/game/node/item/Item.java','org/jetbrains/annotations/NotNull.java','core/tools/Log.java']:
  adapter(path,read(ROOT/'oracle/doors/adapters'/path))
 adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player;public class Player{}')
 adapter('core/api/ContentAPIKt.java','package core.api;public class ContentAPIKt{public static void log(Object o,core.tools.Log l,String s){}public static void setVarbit(Object p,int id,int v){}public static void setVarp(Object p,int id,int v){}}')
 definition=read(ROOT/'oracle/doors/adapters/core/cache/def/impl/SceneryDefinition.java').replace('return defs.computeIfAbsent(id,','if(oracle.Support.failId!=null&&id==oracle.Support.failId)throw new oracle.Support.Failure();return defs.computeIfAbsent(id,')
 adapter('core/cache/def/impl/SceneryDefinition.java',definition)
 direction=read(SERVER/'core/game/world/map/Direction.java');adapter('core/game/world/map/Direction.java','package core.game.world.map;public enum Direction{NORTH_WEST(7),NORTH(0),NORTH_EAST(4),WEST(3),EAST(1),SOUTH_WEST(6),SOUTH(2),SOUTH_EAST(5);public final int value;Direction(int v){value=v;}public int toInteger(){return value;}'+c.method(direction,'public static Direction get(int rotation)')+'}')
 location=read(SERVER/'core/game/world/map/Location.kt');nested=c.method(location,'value class LocationHash(')
 adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val chunkOffsetX:Int get()=x and 7;val chunkOffsetY:Int get()=y and 7;fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);'+c.method(location,'override fun equals(other: Any?)')+'\n'+next(line.strip() for line in location.splitlines() if 'override fun toString()' in line)+'\noverride fun hashCode():Int=hash.hash;companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)}\n@JvmInline '+nested+'}')
 adapter('core/game/world/map/build/ChunkFlags.java','package core.game.world.map.build;public class ChunkFlags{public int baseX,baseY,baseZ;public ChunkFlags(int x,int y,int z){this(x,y,z,false);}public ChunkFlags(int x,int y,int z,boolean p){baseX=x;baseY=y;baseZ=z;}}')
 chunk=read(SERVER/'core/game/world/map/RegionChunk.java')
 signatures=['public RegionChunk(Location base, int rotation)','public void addStatDyn(','public int add(Scenery object)','private void copyObjects(','public RegionChunk copy()','public void rebaseObjects()','private void rebaseObjects(','public static int[] getRotatedPosition(','public int getIndex(','public Scenery[][][] getObjects()','public Scenery[][][] getStatObjects()','public Location getBase()','public void setBase(','public Location getCurrentBase()','public void setCurrentBase(','public int getRotation()','public void setRotation(','public void resetClippingFlags()','public ChunkFlags getFlags()','public ChunkFlags getProjectileFlags()']
 adapter('core/game/world/map/RegionChunk.java','package core.game.world.map;import core.game.node.scenery.*;import core.game.world.map.build.ChunkFlags;public class RegionChunk{public static final int SIZE=8,ARRAY_SIZE=4,NORTH_ROTATION=0,EAST_ROTATION=1,SOUTH_ROTATION=2,WEST_ROTATION=3;protected Location base,currentBase;protected int rotation;protected Scenery[][][]statObjects,objects;private ChunkFlags flags,projectileFlags;'+''.join(c.method(chunk,s)for s in signatures)+'}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases)
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(cases),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim RegionChunk constructor, static/dynamic scenery storage, index, copy/rebase, rotated-position and clipping-reset methods; complete unchanged Scenery, Constructed and Pulse. Definition factory, Node and chunk-flag records are explicit adapters. Scenery/location identity, sparse array contents, defaults, mutation order and constructor failures are compared. No landscape/collision behavior, entity/item lists, dynamic-region rotation, synchronization or live world-session claim.')
 folder=ROOT/'fixtures/chunk'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Chunk fixtures/provenance differ')
  print(f'{len(out)} original chunk-storage scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original chunk-storage scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
