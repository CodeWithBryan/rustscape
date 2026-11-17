"""Verbatim Region.load overloads with explicit cache/parser/scenery boundaries."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/region-load-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(kind,**kw):return dict(op=kind,**kw)
def requests():
 cases=[]
 def add(actions=None,**kw):cases.append(dict(actions=actions or[op('load')],**kw))
 for loaded in [False,True]:
  for build in [False,True]:
   for requested in [None,False,True]:
    for mid in [-1,0,5]:
     for template in [None,-1,12850]:
      add([op('load',**({}if requested is None else dict(build=requested)))],loaded=loaded,build=build,mapId=mid,**({}if template is None else dict(template=template)))
 for data in [None,'','00','000000','00000000']:
  for mid in [-1,5]:
   for template in [None,12850]:add(landData=data,mapId=mid,**({}if template is None else dict(template=template)))
 for data in [None,'','00']:add(mapData=data)
 for event in ['archive','mapBytes','mapParse','keys','landBytes','landParse','clip','object','remove','add']:
  for at in [1,2]:add(failEvent=event,failAt=at,count=2147483647)
 for event in ['mapParse','landParse','clip','object','remove','add']:
  for action in [op('load'),op('state',loaded=False,flags=False,build=True),op('nullChunks'),op('clear',add=False),op('clear',add=True),op('push',add=False,node='c'),op('push',add=True,node='a'),op('erase',add=False,index=1)]:
   add(hooks=[dict(event=event,at=1,actions=[action])])
 for point in [[0,0,0],[0,1,2],[7,7,3]]:add(nulls=[point])
 add(nullChunks=True);add(actual=None)
 add([op('load'),op('load'),op('load',build=True),op('load'),op('load',build=False)])
 add([op('load'),op('input',key='landId',value=-1),op('load',build=True)])
 return cases
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'))
 for part in ['server','client']:
  p=ROOT/'reference'/part
  if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
  run(['git','-C',p,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 java=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 source=read(SERVER/'core/game/world/map/Region.java')
 fields='public static final int PLANES=4,SIZE=64,CHUNKS_SIZE=8;public RegionChunk[][][]chunks=new RegionChunk[8][8][4];public boolean hasFlags;private boolean loaded,build;private int count;public final ArrayList<Scenery>addSceneries=new ArrayList<>(),removeSceneries=new ArrayList<>();public Region(){for(int x=0;x<8;x++)for(int y=0;y<8;y++)for(int z=0;z<4;z++)chunks[x][y][z]=new RegionChunk();}public int getX(){return 50;}public int getY(){return 50;}public int getId(){return(50<<8)|50;}public int getObjectCount(){return count;}public void setObjectCount(int n){count=n;}public boolean isLoaded(){return loaded;}public void setLoaded(boolean b){loaded=b;}public boolean isBuild(){return build;}public void setBuild(boolean b){build=b;}'
 adapter('core/game/world/map/Region.java','package core.game.world.map;import java.util.*;import java.nio.ByteBuffer;import core.cache.Cache;import core.game.world.map.build.*;import core.game.node.scenery.*;import core.game.system.config.XteaParser;public class Region{'+fields+c.method(source,'public static void load(Region r)')+c.method(source,'public static void load(Region r, boolean build)')+'}')
 adapter('core/game/world/map/build/DynamicRegion.java','package core.game.world.map.build;public class DynamicRegion extends core.game.world.map.Region{int id;public DynamicRegion(int i){id=i;}public int getRegionId(){return id;}}')
 adapter('core/game/world/map/RegionChunk.java','package core.game.world.map;public class RegionChunk{public static final int SIZE=8;public Flags flags=new Flags(),projectile=new Flags();public Flags getFlags(){return flags;}public static class Flags{public boolean[][]landscape=new boolean[8][8];public Flags(){landscape[0][0]=true;}public void setLandscape(boolean[][]b){landscape=b;}}}')
 adapter('core/game/world/map/Location.java','package core.game.world.map;public class Location{int x,y,z;public Location(int a,int b,int c){x=a;y=b;z=c;}public int getX(){return x;}public int getY(){return y;}public int getZ(){return z;}}')
 adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;import core.game.world.map.Location;public class Scenery{int id,type;Location l;public Scenery(int i,Location p,int t){id=i;l=p;type=t;}public int getId(){return id;}public int getType(){return type;}public Location getLocation(){return l;}}')
 adapter('core/cache/Cache.java','package core.cache;public class Cache{public static Index[]getIndexes(){Index[]a=new Index[6];a[5]=new Index();return a;}public static class Index{public int getArchiveId(String s){return oracle.Support.archive(s);}public CacheFile getCacheFile(){return new CacheFile();}public byte[]getFileData(int id,int file,int[]keys){return oracle.Support.landBytes(id,keys);}}public static class CacheFile{public byte[]getContainerUnpackedData(int id){return oracle.Support.mapBytes(id);}}}')
 adapter('core/game/system/config/XteaParser.java','package core.game.system.config;public class XteaParser{public static XteaParser Companion=new XteaParser();public int[]getRegionXTEA(int id){return oracle.Support.keys(id);}}')
 adapter('core/game/world/map/build/MapscapeParser.java','package core.game.world.map.build;import core.game.world.map.*;import java.nio.ByteBuffer;public class MapscapeParser{public static void parse(Region r,byte[][][]m,ByteBuffer b){oracle.Support.parseMap(r,m,b);}public static void clipMapscape(Region r,RegionChunk[][][]c,byte[][][]m){oracle.Support.clip(r,c,m);}}')
 adapter('core/game/world/map/build/LandscapeParser.java','package core.game.world.map.build;import core.game.world.map.*;import java.nio.ByteBuffer;public class LandscapeParser{public static void parse(Region r,RegionChunk[][][]c,byte[][][]m,ByteBuffer b,boolean build){oracle.Support.parseLand(r,c,m,b,build);}}')
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;import core.game.node.scenery.Scenery;public class RegionManager{public static Scenery getObject(int x,int y,int z,int id,int t){return oracle.Support.object(x,y,z,id,t);}}')
 adapter('core/game/node/scenery/SceneryBuilder.java','package core.game.node.scenery;public class SceneryBuilder{public static boolean remove(Scenery h){oracle.Support.remove(h);return true;}public static Scenery add(Scenery h){oracle.Support.add(h);return h;}}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,jar]));run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java]);cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=180)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim original Region.load overloads with explicit region/chunk/scenery records and cache/XTEA/parser/scenery callbacks. Compares loaded/build/hasFlags publication, landscape-array reset identities, parser input sharing, short/missing data, dynamic template selection, nested loads, callback mutations, ArrayList iteration and normalized exception roots. Real cache/terrain/scenery parsing and services have separate corpora; actual region delivery, routing and live world entry are not proved here.')
 folder=ROOT/'fixtures/region-load'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Region-load fixtures/provenance differ')
  print(f'{len(out)} original region-load scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original region-load scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
