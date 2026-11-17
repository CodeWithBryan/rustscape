"""Run unchanged SceneryBuilder and its pulse bodies with explicit landscape/chunk bindings."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/scenery-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run

def requests():
 cases=[];rng=random.Random(530)
 base={'a':dict(id=12348,location=[3207,3217,0],rotation=1),'b':dict(id=10327,location=[3207,3218,0],rotation=2),'c':dict(id=12348,location=[3207,3217,0],rotation=1,constructed=True,replaced='a'),'w':dict(id=12348,location=[3207,3217,0],rotation=1,wrapper='a'),'nil':None}
 def op(operation,**kw):return dict(op=operation,**kw)
 def add(actions,**kw):cases.append(dict(nodes=json.loads(json.dumps(kw.pop('nodes',base))),actions=actions,**kw))
 for clip in [False,True]:
  for permanent in [False,True]:
   for current in ['a','c',None]:
    for construct in ['a','b','c','nil']:
     add([op('swap',node='a',other=construct,clip=clip,permanent=permanent),op('flush')],removeResult=current)
 for ticks in [-2,-1,0,1,2,5,2**31-1]:
  for clip in [False,True]:
   for construct in ['a','b','nil']:
    add([op('temporary',node='c',other=construct,clip=clip,ticks=ticks),op('tick'),op('tick'),op('tick'),op('flush')],removeResult='c')
 for node in ['a','c','w','nil']:
  for ticks in [-1,0,1,2]:
   add([op('add',node=node,ticks=ticks,items='i'),op('setItem',items='i',index=1,value=9),op('tick'),op('tick'),op('flush')],items={'i':[1,None,2]})
   add([op('remove',node=node,ticks=ticks),op('tick'),op('tick'),op('flush')])
 for event in ['remove','add','chunk','flag','stop','submit','ground','custom']:
  for action in [op('fail'),op('active',node='a',value=False),op('wrapper',node='b',other='c'),op('replaced',node='c',other='b'),op('restore',node='a',pulse='p'),op('setItem',items='i',index=1,value=9),op('rotate',node='b',value=3),op('update',nodes=['a',None,'b'])]:
   nodes=json.loads(json.dumps(base));nodes['a']['restore']='p';nodes['a']['destruction']='d'
   add([op('swap',node='a',other='b'),op('add',node='a',ticks=1,items='i'),op('tick'),op('temporary',node='c',other='a',ticks=1),op('tick'),op('remove',node='w',ticks=1),op('tick'),op('flush')],nodes=nodes,pulses={'p':{},'d':{}},items={'i':[1,None,2]},hooks=[dict(event=event,at=1,actions=[action])])
 for stopped in [False,True]:
  add([op('add',node='a',ticks=1,items='i'),op('active',node='a',value=stopped),op('tick'),op('flush')],items={'i':[None,4]})
 add([op('temporary',node='a',other='c',ticks=0),op('replaced',node='c',other='b'),op('tick'),op('flush')])
 add([op('swap',node='w',other='b',clip=False),op('location',node='w',value=[3210,3220,1]),op('active',node='w',value=False),op('flush')])
 for _ in range(50):
  actions=[]
  for _ in range(15):
   name=rng.choice(['swap','temporary','add','remove','active','renderable','wrapper','replaced','restore','destruction','rotate','tick','update','flush'])
   actions.append(op(name,node=rng.choice(list(base)),other=rng.choice(list(base)),nodes=rng.sample(list(base),3),pulse='p',ticks=rng.choice([-1,0,1,2,10]),clip=bool(rng.randrange(2)),permanent=bool(rng.randrange(2)),value=bool(rng.randrange(2))if name in ['active','renderable']else rng.randrange(4)))
  add(actions,pulses={'p':{}})
 return cases

def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'))
 for part in ['server','client']:
  p=ROOT/'reference'/part
  if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
  run(['git','-C',p,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);java=[];kt=[];jars=c.artifacts();jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(java if name.endswith('.java')else kt).append(p)
 paths=['core/game/node/scenery/SceneryBuilder.java','core/game/node/scenery/Scenery.java','core/game/node/scenery/Constructed.java','core/game/system/task/Pulse.java','core/game/world/update/flag/UpdateFlag.java','core/game/world/update/flag/chunk/ObjectUpdateFlag.java','core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java','core/net/packet/OutgoingPacket.java','core/net/packet/Context.java','core/net/packet/out/ConstructScenery.java','core/net/packet/out/ClearScenery.java','core/net/packet/out/UpdateAreaPosition.java','core/net/packet/context/BuildSceneryContext.java','core/net/packet/context/AreaPositionContext.java','core/net/event/GameWriteEvent.java','core/net/IoWriteEvent.java']
 for path in paths:p=SERVER/path;read(p);java.append(p)
 skip=['SceneryBuilder.java','RegionManager.java','Player.java','ContentAPIKt.java','Logger.kt','ServerConstants.java','Sounds.java','Pathfinder.java']
 for p in sorted((ROOT/'oracle/doors/adapters').rglob('*')):
  if p.is_file()and p.name not in skip:adapter(p.relative_to(ROOT/'oracle/doors/adapters').as_posix(),read(p))
 for p in sorted((HERE/'adapters').rglob('*')):
  if p.is_file():adapter(p.relative_to(HERE/'adapters').as_posix(),read(p))
 direction=read(SERVER/'core/game/world/map/Direction.java');adapter('core/game/world/map/Direction.java','package core.game.world.map;public enum Direction{NORTH_WEST(7),NORTH(0),NORTH_EAST(4),WEST(3),EAST(1),SOUTH_WEST(6),SOUTH(2),SOUTH_EAST(5);public final int value;Direction(int v){value=v;}public int toInteger(){return value;}'+c.method(direction,'public static Direction get(int rotation)')+'}')
 location=read(SERVER/'core/game/world/map/Location.kt');nested=c.method(location,'value class LocationHash(')
 adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val chunkOffsetX:Int get()=x and 7;val chunkOffsetY:Int get()=y and 7;val chunkBase:Location get()=Location(x and -8,y and -8,z);fun getSceneX(l:Location):Int=x;fun getSceneY(l:Location):Int=y;fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);'+c.method(location,'override fun equals(other: Any?)')+'\noverride fun hashCode():Int=hash.hash;companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)}\n@JvmInline '+nested+'}')
 read(HERE/'Support.java');java.append(HERE/'Support.java')
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases)
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(cases),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged SceneryBuilder, Scenery, Constructed, Pulse, ObjectUpdateFlag/UpdateFlag, outgoing scenery/area packets and game writer. Explicit definition, landscape, chunk selection and scene-coordinate adapters; original location packing/equality. Checks replacement/add/remove decisions, aliases, captured pulse bodies, node activity/destruction callbacks, ground-item array aliases, update requests and outgoing bytes. Actual LandscapeParser/RegionChunk storage, collision, region loading, world startup and sessions remain separate.')
 folder=ROOT/'fixtures/scenery'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Scenery fixtures/provenance differ')
  print(f'{len(out)} original scenery-builder scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original scenery-builder scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
