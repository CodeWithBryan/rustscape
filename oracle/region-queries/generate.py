"""Original RegionManager landscape/clipping/water/teleport queries with mutable services."""
import argparse, hashlib, importlib.util, itertools, json, os, pathlib, random, subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
RUN=ROOT/'runs/region-queries-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(kind,**kw):return dict(op=kind,**kw)
QUERIES=['clip','projectile','landscape','teleport','clipped','water']
def requests():
 rows=[]
 masks=[-2147483648,-1,0,1,2,8,32,128,256,0x200000,0x12c0102,0x12c0108,0x12c0120,0x12c0180,
        0x12c01aa,0x1000000,2147483647]+[1<<i for i in range(31)]
 for flag,has,land in itertools.product(masks,[False,True],[None,False,True]):
  rows.append(dict(flag=flag,hasFlags=has,landscape=land,actions=[op(q)for q in QUERIES]))
 for point in [[-1,-1,3],[16384,-16385,-1],[-2147483648,2147483647,4],[7,8,2],[32767,32768,0]]:
  rows.append(dict(location=point,flag=0,actions=[op(q)for q in QUERIES]))
 for land in [[],[None]*8,[[True]*2]*8,[[False]*8]*2]:
  rows.append(dict(landscape=land,actions=[op(q)for q in QUERIES]))
 rows.append(dict(location=None,actions=[op(q)for q in QUERIES]))
 changes=[op('fail'),op('select',value=1),op('has',region=0,value=False),
          op('location',value=[32767,-16385,7]),op('land',chunk=0,value=None),
          op('land',chunk=0,value=False),op('land',chunk=1,value=True),
          op('chunks',region=0,value=1),op('chunks',region=0,value=None),
          op('fill',value=2),op('replace',value=-1),op('query',name='water')]
 for event,at,change,q in itertools.product(['region','load','chunks','flags'],[1,2],changes,QUERIES):
  rows.append(dict(flag=0,actions=[op(q),op(q)],hooks=[dict(event=event,at=at,actions=[change])]))
 rng=random.Random(530)
 for _ in range(128):
  rows.append(dict(flag=rng.randrange(-2**31,2**31),location=[rng.randrange(-65536,65536),rng.randrange(-65536,65536),rng.randrange(-8,8)],actions=[op(q)for q in QUERIES]))
 return rows
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 assert run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit']
 run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
 pin=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=ROOT/'runs/legacy-interaction-oracle/json-simple-1.1.1.jar'
 assert hashlib.sha256(jar.read_bytes()).hexdigest()==pin['sha256']
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 source=read(SERVER/'core/game/world/map/RegionManager.kt')
 signatures=['fun getClippingFlag(loc: Location, projectile: Boolean = false)',
  'private fun getFlagIndex(','fun getWaterClipFlag(loc: Location)','fun isLandscape(loc: Location)',
  'fun isTeleportPermitted(loc: Location)','fun isClipped(loc: Location)']
 methods='\n'.join(('@JvmStatic 'if not s.startswith('private')else'')+c.method(source,s)for s in signatures)
 original_flags=c.method(source,'fun getFlags(chunkId: Int, projectile: Boolean)').replace('fun getFlags(', 'fun originalFlags(',1)
 adapter('core/game/world/map/RegionManager.kt','''package core.game.world.map
object RegionManager{
@JvmField val CLIPPING_FLAGS=java.util.HashMap<Int,Array<Int>>()
@JvmField val PROJECTILE_FLAGS=java.util.HashMap<Int,Array<Int>>()
@JvmStatic fun forId(id:Int):Region=oracle.Support.region(id)
@JvmStatic fun getFlags(id:Int,p:Boolean):Array<Int> = oracle.Support.flags(id,p)
@JvmStatic fun addClippingFlag(z:Int,x:Int,y:Int,p:Boolean,f:Int){throw UnsupportedOperationException()}
@JvmStatic fun removeClippingFlag(z:Int,x:Int,y:Int,p:Boolean,f:Int){throw UnsupportedOperationException()}
@JvmStatic '''+original_flags+'\n'+methods+'\n}')
 loc=read(SERVER/'core/game/world/map/Location.kt')
 names=['region:','regionId:','chunk:','localX:','localY:','absChunkX:','absChunkY:','chunkX:','chunkY:','chunkOffsetX:','chunkOffsetY:']
 properties='\n'.join(next(line.strip().split(' //')[0]for line in loc.splitlines()if line.strip().startswith('val '+name))for name in names)
 adapter('core/game/world/map/Location.kt','''package core.game.world.map
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4)
val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z
fun moveTo(x:Int,y:Int,z:Int){hash=LocationHash(x,y,(z%4+4)%4)}
'''+properties+'\n}\n@JvmInline '+c.method(loc,'value class LocationHash('))
 adapter('core/game/world/map/Region.java','''package core.game.world.map;
public class Region{public int key;public boolean has;private RegionChunk[][][]chunks;
public Region(int k){key=k;}public boolean isHasFlags(){return has;}
public RegionChunk[][][]getChunks(){RegionChunk[][][]a=chunks;oracle.Support.emit("chunks",key);return a;}
public void setChunks(RegionChunk[][][]a){chunks=a;}public RegionChunk[][][]raw(){return chunks;}
public static void load(Region r){oracle.Support.emit("load",r.key);}}''')
 adapter('core/game/world/map/RegionChunk.java','''package core.game.world.map;
public class RegionChunk{public static final int SIZE=8;public core.game.world.map.build.ChunkFlags flags;
public RegionChunk(){flags=new core.game.world.map.build.ChunkFlags(999,1000,2,false);}}''')
 p=SERVER/'core/game/world/map/build/ChunkFlags.java';read(p);java.append(p)
 p=HERE/'Support.java';read(p);java.append(p);cp=os.pathsep.join(map(str,[classes,std,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',os.pathsep.join(map(str,[std,jar])),'-d',classes,*kt,*java])
 run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8')
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
 rows=[json.loads(x)for x in result.stdout.splitlines()if x];assert len(rows)==len(cases)
 assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Unchanged selected RegionManager clipping/landscape/teleport/clipped/water bodies and original getFlags body under recording wrapper; original Location getters/hash and complete ChunkFlags. Region lookup/load and chunk arrays are explicit mutable recording services. Captured flags, repeated chunk reads, location mutations, null/jagged landscapes, callback reentry/failures and signed masks are observed. Owning-world Scene integration has separate native tests; no live Take/full RegionManager startup/parallel or capacity claim.')
 folder=ROOT/'fixtures/region-queries';folder.mkdir(exist_ok=True)
 if check:assert (folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text())==provenance;print(f'{len(rows)} original region query scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n');print(f'Generated {len(rows)} original region query scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
