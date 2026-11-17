"""Complete unchanged MapscapeParser/ChunkFlags with explicit region/storage records."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/mapscape-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
 out=[];rng=random.Random(530)
 def add(data=None,clip=True,**kw):out.append(dict(actions=([dict(op='parse',data=bytes(data).hex())]if data is not None else[])+([dict(op='clip')]if clip else[]),**kw))
 for value in range(256):
  head=[0]if value==0 else[1,255]if value==1 else[value,255,0]if value<=49 else[value,0]
  add(head+[0]*16383)
 for end in range(9):add([2,255,50,82,1,128,0,0][:end])
 for null in [False,True]:
  for initial in [-2147483648,-1,0,128,0x200000,2147483647]:add([0]*16384,initialFlag=initial,nullRegion=null)
 for point in [[0,0,0],[7,7,3],[4,2,1]]:add([2,1,0]+[0]*16383,nulls=[point]);add(None,nulls=[point])
 for val in [0,1,2,3,4,31,32,255]:
  for separate in [False,True]:add(None,separate=separate,map=[[z,7,7,val]for z in range(4)]+[[1,7,7,2]])
 for fail in [1,2,3,127,128,129,32767,32768]:add(None,failAt=fail,map=[[0,0,0,1],[1,0,0,3]])
 for at in [1,2,3,127,128,129]:
  add(None,mutations=[dict(at=at,z=0,x=0,y=0,value=1),dict(at=at,z=1,x=0,y=0,value=2)])
  add(None,nullRegionAt=at,map=[[0,0,0,1],[0,0,1,1]])
 for val in [0,1,255]:add([2,val,0]+[0]*16383,nullLandscape=True)
 add([2,1,0]+[0]*16383,aliasLandscape=True)
 for _ in range(20):
  tiles={rng.randrange(16384):[rng.randrange(50,82),rng.randrange(82,256),2,rng.randrange(256),0]for _ in range(32)}
  data=sum((tiles.get(i,[0])for i in range(16384)),[])
  add(data,separate=bool(rng.randrange(2)))
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
 java=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 for p in [SERVER/'core/game/world/map/build/MapscapeParser.java',SERVER/'core/game/world/map/build/ChunkFlags.java',HERE/'Support.java']:read(p);java.append(p)
 chunk=read(SERVER/'core/game/world/map/RegionChunk.java');adapter('core/game/world/map/RegionChunk.java','package core.game.world.map;import core.game.world.map.build.ChunkFlags;import core.game.node.scenery.Scenery;public class RegionChunk{public static final int SIZE=8,ARRAY_SIZE=4;protected Location base,currentBase;protected int rotation;protected Scenery[][][]objects,statObjects;private ChunkFlags flags,projectileFlags;'+''.join(c.method(chunk,s)for s in ['public RegionChunk(Location base, int rotation)','public Location getCurrentBase()','public ChunkFlags getFlags()','public ChunkFlags getProjectileFlags()'])+'}')
 adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;public class Scenery{}')
 adapter('core/game/world/map/Location.java','package core.game.world.map;public class Location{int x,y,z;public Location(int a,int b,int c){x=a;y=b;z=c;}public int getX(){return x;}public int getY(){return y;}public int getZ(){return z;}}')
 adapter('core/game/world/map/Region.java','package core.game.world.map;public class Region{public static final int SIZE=64,PLANES=4;public RegionChunk[][][]chunks;public RegionChunk[][][]getChunks(){return oracle.Support.chunks();}}')
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static Integer[]getFlags(int k,boolean p){return oracle.Support.flags(k,p);}public static void addClippingFlag(int z,int x,int y,boolean p,int m){throw new UnsupportedOperationException();}public static void removeClippingFlag(int z,int x,int y,boolean p,int m){throw new UnsupportedOperationException();}}')
 cp=os.pathsep.join(map(str,[classes,std,jar]));run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java]);cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx1g','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=180)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged MapscapeParser and ChunkFlags, verbatim RegionChunk constructor/flag accessors, explicit location/region/storage adapters. Full terrain bytes, both chunk tables/landscape arrays and every allocated clipping flag enter a canonical big-endian state SHA256. Positions, getter/flag call counts, null regions/chunks/landscapes, aliases, separate source/destination regions and partial failures are compared. Region loading/cache delivery, projectile routing, dynamic-region ownership and live world sessions remain separate.')
 folder=ROOT/'fixtures/mapscape'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Mapscape fixtures/provenance differ')
  print(f'{len(out)} original mapscape scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original mapscape scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
