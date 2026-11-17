"""Original LandscapeParser with real scenery storage and ChunkFlags kernels."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/landscape-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
read,run=c.read,c.run
def op(kind,**kw):return dict(op=kind,**kw)
def smart(n):return[n]if n<128 else[(n+32768)>>8,(n+32768)&255]
def requests():
 cases=[];rng=random.Random(530)
 for shape in [0,1,2,3,4,8,9,10,21,22,23]:
  for clip in [0,1,2]:
   for projectile in [False,True]:
    for landscape in [False,True]:
     cases.append(dict(nodes={'a':dict(id=12348,shape=shape,rotation=rng.randrange(4))},definitions={'12348':dict(clip=clip,projectile=projectile,sx=2,sy=3,actions=bool(rng.randrange(2)),interactable=rng.choice([-1,0,1]),second=bool(rng.randrange(2)))},actions=[op('add',node='a',landscape=landscape),op('remove',node='a'),op('remove',node='a'),op('add',node='a')]))
 for xy in [[3200,3200,0],[3207,3207,0],[3263,3263,3],[3264,3200,0],[-1,-1,2]]:
  for rot in range(4):
   cases.append(dict(nodes={'a':dict(id=1,location=xy,shape=2,rotation=rot)},actions=[op('flag',node='a',landscape=True,store=True),op('remove',node='a'),op('renderable',node='a',value=False),op('add',node='a'),op('remove',node='a')]))
 cases.append(dict(nodes={'a':dict(id=1,shape=0),'b':dict(id=1,shape=22)},actions=[op('add',node='a'),op('remove',node='b'),op('remove',node='a')]))
 for x,y in [(-1,0),(0,-1),(8,0),(0,8)]:cases.append(dict(nodes={'a':dict(id=1,shape=22)},actions=[op('flag',node='a',x=x,y=y,store=True,landscape=True),op('apply',node='a',x=x,y=y)]))
 # Original delta/smart landscape stream: a wall and floor object, including a bridge.
 data=smart(12349)+smart((7<<6|17)+1)+[1]+[0]+smart(1)+smart((1<<12|8<<6|17)+1)+[(22<<2)|2]+[0,0]
 for bridge in [[],[[7,17]],[[8,17]],[[7,17],[8,17]]]:
  for store in [False,True]:
   cases.append(dict(nodes={},bridges=bridge,actions=[op('parse',data=data,store=store)]))
 for end in range(len(data)):cases.append(dict(nodes={},actions=[op('parse',data=data[:end],store=True)]))
 for delta in [[255,255,0],[255,255,255,255,1]]:cases.append(dict(nodes={},count=2147483647,actions=[op('parse',data=delta+smart(1)+[0,0,0],store=True)]))
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
 for path in ['core/game/node/scenery/Scenery.java','core/game/node/scenery/Constructed.java','core/game/system/task/Pulse.java','core/game/world/map/build/ChunkFlags.java','core/game/world/map/build/LandscapeParser.java']:
  p=SERVER/path;read(p);java.append(p)
 for path in ['core/game/node/Node.java','core/game/node/entity/impl/GameAttributes.java','core/game/interaction/InteractPlugin.java','core/game/interaction/DestinationFlag.java','core/cache/def/impl/VarbitDefinition.java','core/game/node/item/Item.java','org/jetbrains/annotations/NotNull.java','core/tools/Log.java']:
  adapter(path,read(ROOT/'oracle/doors/adapters'/path))
 adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player;public class Player{}')
 adapter('core/api/ContentAPIKt.java','package core.api;public class ContentAPIKt{public static void log(Object o,core.tools.Log l,String s){}public static void setVarbit(Object p,int id,int v){}public static void setVarp(Object p,int id,int v){}}')
 definition=read(ROOT/'oracle/doors/adapters/core/cache/def/impl/SceneryDefinition.java').replace('public int id,sizeX=1,sizeY=1;','public int id,sizeX=1,sizeY=1,clipType=2,interactable=-1;public boolean secondBool,projectileClipped=true,actions=true;public boolean isProjectileClipped(){return projectileClipped;}public boolean hasActions(){return actions;}')
 adapter('core/cache/def/impl/SceneryDefinition.java',definition)
 direction=read(SERVER/'core/game/world/map/Direction.java');adapter('core/game/world/map/Direction.java','package core.game.world.map;public enum Direction{NORTH_WEST(7),NORTH(0),NORTH_EAST(4),WEST(3),EAST(1),SOUTH_WEST(6),SOUTH(2),SOUTH_EAST(5);public final int value;Direction(int v){value=v;}public int toInteger(){return value;}'+c.method(direction,'public static Direction get(int rotation)')+'}')
 location=read(SERVER/'core/game/world/map/Location.kt');nested=c.method(location,'value class LocationHash(')
 adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val chunkOffsetX:Int get()=x and 7;val chunkOffsetY:Int get()=y and 7;val chunkX:Int get()=(x and 63) shr 3;val chunkY:Int get()=(y and 63) shr 3;val regionId:Int get()=((x shr 6) shl 8) or (y shr 6);val chunkBase:Location get()=Location(x and -8,y and -8,z);fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);'+c.method(location,'override fun equals(other: Any?)')+'\n'+next(line.strip()for line in location.splitlines()if'override fun toString()'in line)+'\noverride fun hashCode():Int=hash.hash;companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)}\n@JvmInline '+nested+'}')
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static Region forId(int id){return oracle.Support.forId(id);}public static RegionChunk getRegionChunk(Location l){return forId(l.getRegionId()).getChunks()[l.getChunkX()][l.getChunkY()][l.getZ()];}public static Integer[]getFlags(int key,boolean p){return oracle.Support.flags(key,p);}public static void addClippingFlag(int z,int x,int y,boolean p,int m){oracle.Support.border(z,x,y,p,m,true);}public static void removeClippingFlag(int z,int x,int y,boolean p,int m){oracle.Support.border(z,x,y,p,m,false);}}')
 adapter('core/game/world/map/Region.java','package core.game.world.map;public class Region{private int id,count;public Region(int i){id=i;}public int getX(){return(id>>8)&255;}public int getY(){return id&255;}public int getObjectCount(){return count;}public void setObjectCount(int n){count=n;}public static void load(Region r){}public RegionChunk[][][]getChunks(){return oracle.Support.regionChunks(this);}}')
 utils=read(SERVER/'core/cache/misc/buffer/ByteBufferUtils.kt');adapter('core/cache/misc/buffer/ByteBufferUtils.kt','package core.cache.misc.buffer\nimport java.nio.ByteBuffer\nobject ByteBufferUtils{@JvmStatic '+c.method(utils,'fun getSmart(')+'\n@JvmStatic '+c.method(utils,'fun getBigSmart(')+'}')
 chunk=read(SERVER/'core/game/world/map/RegionChunk.java');signatures=['public RegionChunk(Location base, int rotation)','public void addStatDyn(','public int add(Scenery object)','public int getIndex(','public Scenery[][][] getObjects()','public Scenery[][][] getStatObjects()','public Location getCurrentBase()','public ChunkFlags getFlags()','public ChunkFlags getProjectileFlags()','public Region getRegion()']
 adapter('core/game/world/map/RegionChunk.java','package core.game.world.map;import core.game.node.scenery.*;import core.game.world.map.build.ChunkFlags;public class RegionChunk{public static final int SIZE=8,ARRAY_SIZE=4;protected Location base,currentBase;protected int rotation;protected Scenery[][][]statObjects,objects;private ChunkFlags flags,projectileFlags;'+''.join(c.method(chunk,s)for s in signatures)+'}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx512m','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases)
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(cases),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged LandscapeParser, ChunkFlags, Scenery, Constructed and Pulse; verbatim RegionChunk constructor/storage/index/accessors and original smart decoders. Definition fields/child actions, region load/storage and border routing are explicit adapters. Compares placement/removal storage, wall/object/floor movement+projectile masks, bridge decoding, counts, stream positions and partial malformed-input effects. No original RegionManager loading/routing, complete SceneryDefinition cache decoder, reentrant world callbacks, SceneryBuilder/door integration, walking or live session claim.')
 folder=ROOT/'fixtures/landscape'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Landscape fixtures/provenance differ')
  print(f'{len(out)} original landscape scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original landscape scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
