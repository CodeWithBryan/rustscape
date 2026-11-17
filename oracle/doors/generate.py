"""Compile unchanged door handler/configuration and scenery/pulse state with explicit world adapters."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/doors-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run

def requests():
 rng=random.Random(530);cases=[]
 base={'a':dict(id=12348,location=[3207,3217,0],rotation=1),'b':dict(id=12349,location=[3208,3217,0],rotation=1),'c':dict(id=12350,location=[3206,3217,0],rotation=1),'n':dict(id=10327,location=[3207,3218,0],rotation=2,constructed=True,replaced='a')}
 configs=[dict(id=12348,replaceId=10327,fence=False,metal=False),dict(id=12349,replaceId=11382,fence=False,metal=False),dict(id=12350,replaceId=11382,fence=False,metal=False)]
 def op(operation,**kw):return dict(op=operation,**kw)
 def add(actions,**kw):cases.append(dict(nodes=json.loads(json.dumps(kw.pop('nodes',base))),configs=kw.pop('configs',configs),actions=actions,**kw))
 for shape in [0,1,2,3,9,10,22]:
  for rotation in [0,1,2,3,4,7]:
   for second in [None,'b']:
    nodes=json.loads(json.dumps(base));nodes['a'].update(shape=shape,rotation=rotation);nodes['b'].update(shape=shape,rotation=rotation)
    add([op('point',value=rotation),op('end',node='a'),op('destination',node='a'),op('second',node='a'),op('rotations',node='a',second=second,point=[1,0]),op('open',node='a',second=second,id=4577,otherId=11382,fence=bool(rotation&1)),op('handle',node='a')],nodes=nodes,region=['b']if second else[])
 for player in [True,False]:
  for objectid in [3,1530,1512,3628,4545,2112,35549,35551,2406,12348,36917,36919]:
   nodes=json.loads(json.dumps(base));nodes['a']['id']=objectid
   add([op('handle',node='a'),op('autowalk',node='a',end=[3210,3217,0]),op('tick'),op('tick'),op('autowalkFence',node='a',id=700,otherId=701),op('tick'),op('tick'),op('logout')],nodes=nodes,player=player,region=['b'],own=[3207,3217,0])
 for flag in ['metal','fence','autowalk']:
  for value in [True,False]:
   cs=json.loads(json.dumps(configs));cs[0][flag]=value
   add([op('handle',node='a'),op('handle',node='n'),op('tick'),op('tick'),op('logout')],configs=cs,region=[])
 for event in ['audio','replace','object','lock','location','logoutAdd','logoutRemove','walkReset','walkAdd','find','diary','submit','message']:
  for action in [op('fail'),op('rotate',node='a',value=3),op('own',value=[3046,9756,0]),op('configSet',id=12348,field='replaceId',value=4577),op('region',value=[]),op('charge',node='a',value=88)]:
   add([op('destination',node='a'),op('handle',node='a'),op('autowalk',node='a',end=[3209,3217,0]),op('tick'),op('tick'),op('logout'),op('handle',node='n')],hooks=[dict(event=event,at=1,actions=[action])],region=['b'],moveNear=True)
 for offset in [[-1,0],[1,0],[0,-1],[0,1],[0,0]]:
  for rotation in range(4):
   nodes=json.loads(json.dumps(base));nodes['a'].update(id=36917,rotation=rotation);nodes['b'].update(location=[3207+offset[0],3217+offset[1],0],rotation=rotation)
   add([op('autowalkFence',node='a',id=7,otherId=8),op('tick'),op('tick'),op('open',node='a',second='b',id=7,otherId=8,fence=True,clip=False)],nodes=nodes,region=['b'])
 for own in [[3046,9756,0],[3046,9766,0],[3046,9767,0],[3202,3169,0],[3202,3184,0],[3202,3185,0],[3264,3200,0]]:
  for objectid in [2112,35549,35551,2406]:
   add([op('autowalk',node='a',end=own),op('tick'),op('tick'),op('logout')],own=own,nodes={'a':dict(id=objectid,location=[3207,3217,0],rotation=1)},region=[])
 for shape in [0,9]:
  for rotation in range(4):
   for dx in [-1,0,1]:
    for dy in [-1,0,1]:
     for near in [False,True]:
      nodes=json.loads(json.dumps(base));nodes['a'].update(shape=shape,rotation=rotation);nodes['n'].update(shape=shape,close=True)
      add([op('destination',node='a'),op('end',node='a'),op('destination',node='n')],nodes=nodes,own=[3207+dx,3217+dy,0],moveNear=near)
 for text in ['null','{}','[null]','[1]','[{"id":1,"replaceId":2}]','[{"id":1,"replaceId":1,"metal":"TRUE","fence":"true"}]','[{"id":"１２","replaceId":"+2","metal":true},{"id":"bad","replaceId":4}]','[{"id":1,"replaceId":2},{"id":3,"replaceId":2}]','[{"id":2147483648,"replaceId":2}]','[{"id":1.0,"replaceId":2}]','[{"id":2,"replaceId":3,"fence":" true ","metal":1,"autowalk":null}]']:
  add([op('retain',id=12348,name='old'),op('load',text=text),op('retain',id=2,name='new'),op('configSet',id=2,field='replaceId',value=77),op('load',text='[{"id":2,"replaceId":7}]')])
 for closed in [True,False]:
  for region in [[],['b']]:
   for wrapper in [None,'b']:
    nodes=json.loads(json.dumps(base));nodes['a'].update(close=closed,wrapper=wrapper)
    add([op('handle',node='a'),op('handle',node='n'),op('destination',node='n')],nodes=nodes,region=region)
 for metal,fence in [(True,False),(False,True),(False,False)]:
  cs=json.loads(json.dumps(configs));cs[0].update(metal=metal,fence=fence)
  add([op('autowalk',node='a',end=[3209,3217,0]),op('tick'),op('tick')],configs=cs,hooks=[dict(event='audio',actions=[op('configSet',id=12348,field='metal',value=not metal)])])
 for event in ['diary','logoutRemove']:
  add([op('autowalk',node='a',end=[3046,9756,0]),op('tick'),op('tick')],nodes={'a':dict(id=2112,location=[3046,9756,0],rotation=1)},own=[3046,9756,0],hooks=[dict(event=event,actions=[op('fail')])])
 add([op('retain',id=12348,name='a'),op('retain',id=12348,name='b'),op('configSet',id=12348,field='replaceId',value=7),op('load',text='[{"id":12348,"replaceId":9}]'),op('retain',id=12348,name='new')])
 add([op('load',text='[{"id":8,"replaceId":9}]')],hooks=[dict(event='log',actions=[op('fail')])])
 for region in [[],['b']]:
  add([op('handle',node='a'),op('autowalk',node='a',end=[3209,3217,0]),op('tick'),op('tick')],region=region,replaceResult=False)
 full=read(ROOT/'fixtures/doors/door_configs.json');add([op('load',text=full),op('retain',id=10327,name='reverse'),op('handle',node='a')],configs=[],region=[])
 for _ in range(50):
  actions=[]
  for _ in range(15):
   name=rng.choice(['handle','end','destination','second','open','autowalk','autowalkFence','tick','logout','charge','rotate','own'])
   actions.append(op(name,node=rng.choice(list(base)),second=rng.choice([None,'a','b','c']),id=rng.choice([1,4577,10327,36917]),otherId=7,fence=bool(rng.randrange(2)),clip=bool(rng.randrange(2)),value=rng.choice([0,1,2,3,4,7,88,1000])if name!='own'else[3207,3217,0],end=[3209,3217,0]))
  add(actions,region=['a','b','c'])
 return cases

def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'))
 for part in ['server','client']:
  p=ROOT/'reference'/part
  if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
  run(['git','-C',p,'diff','--quiet','HEAD','--'])
 data=run(['git','-C',ROOT/'reference/server','show',lock['server']['commit']+':Server/data/configs/door_configs.json']).stdout
 fixture=ROOT/'fixtures/doors/door_configs.json'
 if check:
  if fixture.read_bytes()!=data:raise RuntimeError('Pinned door configuration differs')
 else:fixture.parent.mkdir(parents=True,exist_ok=True);fixture.write_bytes(data)
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);java=[];kt=[];jars=c.artifacts();jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(java if name.endswith('.java')else kt).append(p)
 for path in ['core/game/global/action/DoorActionHandler.java','core/game/system/config/DoorConfigLoader.kt','core/game/node/scenery/Scenery.java','core/game/node/scenery/Constructed.java','core/game/system/task/Pulse.java']:
  p=SERVER/path;read(p);(java if path.endswith('.java')else kt).append(p)
 for p in sorted((HERE/'adapters').rglob('*')):
  if p.is_file():adapter(p.relative_to(HERE/'adapters').as_posix(),read(p))
 direction=read(SERVER/'core/game/world/map/Direction.java')
 adapter('core/game/world/map/Direction.java','package core.game.world.map;public enum Direction{NORTH_WEST(7),NORTH(0),NORTH_EAST(4),WEST(3),EAST(1),SOUTH_WEST(6),SOUTH(2),SOUTH_EAST(5);public final int value;Direction(int v){value=v;}public int toInteger(){return value;}'+c.method(direction,'public static Direction get(int rotation)')+c.method(direction,'public static Direction getDirection(int diffX, int diffY)')+'}')
 location=read(SERVER/'core/game/world/map/Location.kt');nested=c.method(location,'value class LocationHash(')
 adapter('core/game/world/map/Location.kt','package core.game.world.map\nimport kotlin.math.sqrt\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val regionId:Int get()=x shr 6 shl 8 or(y shr 6);fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);@JvmOverloads fun withinDistance(other:Location,dist:Int=15)=other.z==z&&getDistance(this,other)<=dist;'+c.method(location,'override fun equals(other: Any?)')+'\noverride fun hashCode():Int=hash.hash;companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z);'+c.method(location,'fun getDistance(first: Location, second: Location)')+'}\n@JvmInline '+nested+'}')
 read(HERE/'Support.java');java.append(HERE/'Support.java')
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases)
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'),ensure_ascii=True)+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(cases),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged DoorActionHandler, DoorConfigLoader, Scenery, Constructed and Pulse. Original location packing/distance/equality and direction methods in adapters. Complete pinned door configuration data is compared. Explicit definition, scene replacement, collision, player, walking, logout, clock/scheduler, diary and audio bindings. Replacement requests are observed; they do not establish SceneryBuilder/LandscapeParser or live world integration.')
 folder=ROOT/'fixtures/doors'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Door fixtures/provenance differ')
  print(f'{len(out)} original door scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original door scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)

