"""Complete server definitions/Animation/Graphics/Animator with recording world services."""
import argparse,hashlib,importlib.util,itertools,json,os,pathlib,random,struct,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/animator-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(kind,**kw):return dict(op=kind,**kw)
def seq(durations,high=None):
 n=len(durations);return (bytes([1])+struct.pack('>H',n)+b''.join(struct.pack('>H',v&65535)for v in durations)+b''.join(struct.pack('>H',v)for v in range(n))+b''.join(struct.pack('>H',v&65535)for v in (high or [1]*n))+bytes([0])).hex()
def requests():
 rows=[]
 payloads=[b'\0',bytes([4,14,0]),bytes([3,0,0]),bytes([3,3,0,127,255,0]),bytes([2,255,255,5,255,6,0,1,7,255,255,8,3,9,1,10,2,11,255,0]),bytes([12,3,0,1,0,2,0,3,0,1,128,0,255,255,0]),bytes([13,0,3,0,1,0x80,0xff,0xfe,3,1,2,3,0xff,0xff,0,1,0]),bytes([250,4,251,0])]
 payloads += [bytes.fromhex(seq(v,h))for v,h in [([0,1,30,100,101,65535],[0,1,32767,32768,65535,65534]),([52],None),([],None)]]
 for raw in payloads:
  for length in range(len(raw)+1):
   rows.append(dict(actions=[op('decode',data=raw[:length].hex()),op('finish'),op('lookup',id=128),op('duration',animation=0)],files={'128':raw[:length].hex()}))
 for durations in [None,[],[0],[29],[30],[31],[100],[101],[-1],[-2147483648],[-2147483647,1,30],[100]*1000,[2147483647,-2147483647,99]]:
  rows.append(dict(actions=[op('def_patch',definition=0,durations=durations),op('cache_set',id=1,definition=0),op('duration',animation=0),op('def_patch',definition=0,durations=[90]),op('duration',animation=0),op('id',animation=0,value=2),op('duration',animation=0)]))
 for tick,priority,incoming in itertools.product([-2147483648,-1,0,100,2147483647],[None,0,1,2,3],[None,0,1,2,3]):
  rows.append(dict(tick=tick,actions=[op('priority',value=priority),op('anim_priority',animation=0,value=incoming),op('animate',animation=0,graphic=0),op('priority',value=priority),op('anim_priority',animation=1,value=incoming),op('animate',animation=1,graphic=1),op('is_animating'),op('stop'),op('reset'),op('is_animating')]))
 for owner,clocks,accepted in itertools.product([None,0,1],[None,[],[0],[0]*10],[False,True]):
  rows.append(dict(owner=owner,clocks=clocks,register=accepted,actions=[op('animate',animation=0,graphic=0),op('force',animation=1),op('reset'),op('set_animation',animation=None),op('set_graphics',graphic=1),op('is_animating'),op('animate',animation=None,graphic=0)]))
 changes=[op('fail'),op('clock',value=-2147483648),op('clock',value=101),op('id',animation=0,value=0),op('id',animation=0,value=-1),op('anim_priority',animation=0,value=None),op('anim_priority',animation=0,value=3),op('priority',value=None),op('force',animation=None),op('set_animation',animation=1),op('mask_select',entity=0,value=1),op('register_result',value=False),op('clocks',entity=0,value=[]),op('clocks',entity=0,value=None),op('cache_set',id=1,definition=0),op('def_patch',definition=0,durations=[90]),op('file',id=1,data=seq([100])),op('duration',animation=1)]
 for event,at,change in itertools.product(['tick','file','masks','register'],[1,2],changes):
  rows.append(dict(hooks=[dict(event=event,at=at,actions=[change])],actions=[op('animate',animation=0,graphic=0),op('is_animating'),op('duration',animation=0),op('animate',animation=1,graphic=1),op('force',animation=2),op('reset')]))
 for selected,gr,loc in itertools.product([-1,0,1],[None,0,1],[None,0,1]):
  for event in ['chunk','make_flag','enqueue']:
   rows.append(dict(chunk=selected,hooks=[dict(event=event,at=1,actions=[op('chunk_select',value=1),op('move',location=0,coords=[-1,32768,3])])],actions=[op('send',graphic=gr,location=loc),op('send',graphic=1,location=0)]))
 rng=random.Random(530)
 actions=[op('animate',animation=a,graphic=g)for a,g in itertools.product([None,0,1,2,3,6,7],[None,0,1,2])]+[op('stop'),op('reset'),op('is_animating'),op('force',animation=None),op('force',animation=0),op('set_animation',animation=None),op('priority',value=3),op('id',animation=0,value=0),op('clock',value=101),op('duration',animation=0),op('duration',animation=1),op('cache_clear')]
 for _ in range(96):rows.append(dict(actions=[rng.choice(actions)for _ in range(24)]))
 for row in rows:row.setdefault('files',{'1':seq([30]),'2':seq([90]),'535':seq([52])})
 for id in [-2147483648,-129,-128,-1,0,127,128,2147483647]:
  rows.append(dict(files={str(id):seq([52],[32768])},actions=[op('lookup',id=id),op('lookup',id=id),op('cache_clear'),op('file',id=id,data=''),op('lookup',id=id),op('lookup',id=id),op('file',id=id,data=None),op('lookup',id=id)]))
 rows.append(dict(files={'1':seq([30])},actions=[op('object',animation=0,value=0),op('object',animation=1,value=0),op('object',animation=0,value=1),op('object',animation=0,value=None),op('get_definition',animation=0),op('cache_set',id=2,definition=0),op('id',animation=0,value=2),op('get_definition',animation=0),op('duration',animation=0)]))
 return rows
def generate(check,audit_file=None):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server';assert run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];run(['git','-C',ref,'diff','--quiet','HEAD','--'])
 RUN.mkdir(exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));pin=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=pin['sha256']:
  with urllib.request.urlopen(pin['url'],timeout=60)as response:data=response.read()
  assert len(data)==pin['size']and hashlib.sha256(data).hexdigest()==pin['sha256'];jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 for name in ['core/cache/def/impl/AnimationDefinition.java','core/game/world/update/flag/context/Animation.java','core/game/world/update/flag/context/Graphics.java','core/game/node/entity/impl/Animator.java','core/game/interaction/Clocks.kt','core/cache/misc/buffer/ByteBufferUtils.kt','core/game/world/update/flag/EntityFlag.kt']:
  p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
 adapter('core/cache/Cache.java','''package core.cache;public class Cache{public static Index[]getIndexes(){Index[]a=new Index[21];a[20]=new Index();return a;}public static class Index{public byte[]getFileData(int g,int f){return oracle.Support.file(g,f);}}}''')
 adapter('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{public static int getTicks(){return oracle.Support.now();}}')
 adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;public class Entity{public int id;public int[]clocks=new int[10];public core.game.world.update.UpdateMasks masks;public Entity(int i){id=i;}public core.game.world.update.UpdateMasks getUpdateMasks(){core.game.world.update.UpdateMasks m=masks;oracle.Support.emit("masks",id,m.id);return m;}}''')
 adapter('core/game/world/update/UpdateMasks.java','''package core.game.world.update;public class UpdateMasks{public int id;public UpdateMasks(int i){id=i;}public boolean register(core.game.world.update.flag.EntityFlag f,Object c){return oracle.Support.register(this,f,c);}}''')
 adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;public class Scenery{public int id;public Scenery(int i){id=i;}}')
 location=read(SERVER/'core/game/world/map/Location.kt')
 adapter('core/game/world/map/Location.kt','''package core.game.world.map
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,z);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;fun moveTo(x:Int,y:Int,z:Int){hash=LocationHash(x,y,z)}}
@JvmInline '''+c.method(location,'value class LocationHash('))
 adapter('core/game/world/map/RegionManager.java','''package core.game.world.map;public class RegionManager{public static Chunk getRegionChunk(Location l){return oracle.Support.chunk(l);}public static class Chunk{public int id;public Chunk(int i){id=i;}public void flag(core.game.world.update.flag.chunk.GraphicUpdateFlag f){oracle.Support.enqueue(this,f);}}}''')
 adapter('core/game/world/update/flag/chunk/GraphicUpdateFlag.java','''package core.game.world.update.flag.chunk;public class GraphicUpdateFlag{public core.game.world.update.flag.context.Graphics graphic;public core.game.world.map.Location location;public GraphicUpdateFlag(core.game.world.update.flag.context.Graphics g,core.game.world.map.Location l){graphic=g;location=l;oracle.Support.made(g,l);}}''')
 p=HERE/'Support.java';read(p);java.append(p);cp=os.pathsep.join(map(str,[classes,std,jar]))
 run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',os.pathsep.join(map(str,[std,jar])),'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=[json.loads(audit_file.read_text(encoding='utf-8'))]if audit_file else requests();p=RUN/('audit-input.jsonl'if audit_file else'input.jsonl');p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:out=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=180)
 if out.returncode:raise RuntimeError(out.stderr.decode(errors='replace'))
 rows=[json.loads(x)for x in out.stdout.splitlines()if x];assert len(rows)==len(cases);assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
 if audit_file:
  audit_file.with_name('audit-output.json').write_text(json.dumps(rows[0],separators=(',',':'))+'\n',encoding='utf-8');print('Original full animation archive audit observed.');return
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged AnimationDefinition/Animation/Graphics/Animator/Clocks/ByteBufferUtils and EntityFlag. Actual lazy cache, partial parse fields/positions, durations/overflow, mutable reset/context identity, priorities, deadlines, clock writes, register results and graphic chunk receiver order. Cache index20, world tick, Entity clocks/masks and flag delivery are explicit reentrant recording adapters. Full mask/profile/combat/content and scene graphic delivery remain owning-world integration; native prepared bindings and actual-cache audit are separately scoped.')
 folder=ROOT/'fixtures/animator';folder.mkdir(exist_ok=True)
 if check:assert (folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text())==provenance;print(f'{len(rows)} original animator/definition scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n');print(f'Generated {len(rows)} original animator/definition scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);p.add_argument('--audit-file',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check,a.audit_file)
