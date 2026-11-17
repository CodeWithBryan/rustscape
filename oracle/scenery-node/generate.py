"""Complete original Scenery/Constructed with retained definitions and morph families."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/scenery-node-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(kind,**kw):return dict(op=kind,**kw)
def requests():
 out=[];rng=random.Random(530)
 base={'1':dict(name='Door',sizeX=2,sizeY=3,childrenIds=[2,-1,3,2],configFileId=7,configId=9),'2':dict(name='Door',options=['Open',None,None,None,None]),'3':dict(name='Gate',childrenIds=[4,-1],configFileId=8),'4':dict(name='Gate',sizeX=4,sizeY=2)}
 def add(actions,definitions=None,**kw):out.append(dict(definitions=base if definitions is None else definitions,actions=actions,**kw))
 for rotation in [-5,-4,-1,0,1,2,3,4,5,6,7,8]:
  for built in [False,True]:
   add([op('new',name='root',rotation=rotation,constructed=built),op('size',node='root'),op('center',node='root'),op('occupied',node='root'),op('children',node='root',name='array'),op('viewGet',view='array',index=0,name='kid'),op('child',node='root',name='shown'),op('asConstructed',node='root',name='constructed'),op('permanent',node='constructed')])
 for shape in [-2,-1,0,1,2,3,4,8,9,10,17,21,22,23,255]:
  for id in [-1,0,1,99]:
   add([op('new',id=id,name='root',shape=shape),op('layer',node='root'),op('transform',node='root',id=2,name='copy',rotation=2),op('layer',node='copy')])
 for index in [-1,0,1,2,3,4,2**31-1]:
  for kw in [{},{'missingVarbit':True}]:
   add([op('new',name='root'),op('child',node='root',name='player',player=True),op('child',node='root',name='none'),op('setChildIndex',node='player',value=index),op('setChildIndex',node='root',value=index)],configValue=index,**kw)
 for ids in [None,[],[2],[2,-1],[2,3,-1]]:
  defs={'1':dict(name='Parent',childrenIds=ids,configFileId=-1,configId=9),'2':dict(name='Child',configFileId=-1,configId=11)}
  add([op('new',name='root'),op('child',node='root',name='shown',player=True),op('setChildIndex',node='shown',value=3),op('children',node='root',name='array')],defs,configValue=0)
 add([op('new',name='root'),op('children',node='root',name='old'),op('viewGet',view='old',index=0,name='kid'),op('setDef',id=1,fields={'name':'Renamed','sizeX':5,'sizeY':1,'options':['Close',None,None,None,None],'childrenIds':None}),op('size',node='root'),op('center',node='root'),op('rotate',node='root',value=3),op('size',node='root'),op('occupied',node='root'),op('replaceDef',id=1,fields={'id':0,'name':'Replacement'}),op('transform',node='root',id=1,name='new'),op('child',node='root',name='chosen'),op('setChildIndex',node='kid',value=5)])
 add([op('new',id=2,name='root'),op('asConstructed',node='root',name='built'),op('active',node='root',value=False),op('renderable',node='root',value=False),op('charge',node='root',value=88),op('transform',node='root',id=2,name='copy'),op('equals',node='copy',other='root'),op('asConstructed',node='built',name='same'),op('location',node='root',value=[3217,3207,1]),op('equals',node='copy',other='root')])
 for sx,sy in [(-1,0),(0,0),(1,1),(2,3),(5,2)]:
  for rotation in range(8):add([op('new',id=1,name='root',rotation=rotation),op('size',node='root'),op('occupied',node='root'),op('rotate',node='root',value=rotation+1),op('size',node='root'),op('occupied',node='root')],{'1':dict(sizeX=sx,sizeY=sy)})
 add([op('new',name='root'),op('child',node='root',name='kid'),op('wrapper',node='kid',other=None),op('setChildIndex',node='kid',value=4),op('wrapper',node='kid',other='root'),op('setChildIndex',node='kid',value=4)])
 for _ in range(30):
  actions=[op('new',name='root')]
  for i in range(12):
   kind=rng.choice(['transform','asConstructed','child','rotate','location','size','center','occupied','permanent','setChildIndex'])
   actions.append(op(kind,node='root',name=f'n{i}',id=rng.choice([-1,0,1,2,3,4]),rotation=rng.randrange(8),location=[rng.randrange(-32768,32768),rng.randrange(-32768,32768),rng.randrange(-1,5)],player=bool(rng.randrange(2)),value=[rng.randrange(-32768,32768),rng.randrange(-32768,32768),rng.randrange(-1,5)] if kind=='location' else rng.randrange(-1,9)))
  add(actions,configValue=rng.randrange(-1,5))
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
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if name.endswith('.kt')else java).append(p)
 for path in ['core/cache/def/impl/SceneryDefinition.java','core/cache/def/Definition.java','core/cache/misc/buffer/ByteBufferUtils.kt','core/game/node/scenery/Scenery.java','core/game/node/scenery/Constructed.java','core/game/system/task/Pulse.java']:
  p=SERVER/path;read(p);(kt if p.suffix=='.kt'else java).append(p)
 for path in ['core/game/node/Node.java','core/game/node/entity/impl/GameAttributes.java','core/game/interaction/InteractPlugin.java','core/game/interaction/DestinationFlag.java','core/game/node/item/Item.java','org/jetbrains/annotations/NotNull.java','core/tools/Log.java']:
  adapter(path,read(ROOT/'oracle/doors/adapters'/path))
 strings=read(SERVER/'core/tools/StringUtils.java');adapter('core/tools/StringUtils.java','package core.tools;public class StringUtils{'+c.method(strings,'public static boolean isPlusN(')+'}')
 adapter('core/game/interaction/OptionHandler.java','package core.game.interaction;public class OptionHandler{}')
 adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player;public class Player{}')
 adapter('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{public static void prompt(boolean b){throw new UnsupportedOperationException();}}')
 adapter('core/api/ContentAPIKt.java','package core.api;import core.game.node.entity.player.Player;public class ContentAPIKt{public static void log(Object o,core.tools.Log l,String s){oracle.Support.log(o,s);}public static int getVarp(Player p,int id){return oracle.Support.varp(id);}public static void setVarp(Player p,int id,int v){oracle.Support.setVarp(id,v);}public static void setVarbit(Player p,int id,int v){oracle.Support.setVarbit(id,v);}}')
 adapter('core/cache/def/impl/VarbitDefinition.java','package core.cache.def.impl;public class VarbitDefinition{int value;VarbitDefinition(int v){value=v;}public static VarbitDefinition forObjectID(int id){Integer v=oracle.Support.varbit(id);return v==null?null:new VarbitDefinition(v);}public int getValue(core.game.node.entity.player.Player p){return value;}}')
 adapter('core/cache/Cache.java','package core.cache;public class Cache{public static int getObjectDefinitionsSize(){throw new UnsupportedOperationException();}public static Index[]getIndexes(){throw new UnsupportedOperationException();}public static class Index{public byte[]getFileData(int g,int f){throw new UnsupportedOperationException();}}}')
 direction=read(SERVER/'core/game/world/map/Direction.java');adapter('core/game/world/map/Direction.java','package core.game.world.map;public enum Direction{NORTH_WEST(7),NORTH(0),NORTH_EAST(4),WEST(3),EAST(1),SOUTH_WEST(6),SOUTH(2),SOUTH_EAST(5);public final int value;Direction(int v){value=v;}public int toInteger(){return value;}'+c.method(direction,'public static Direction get(int rotation)')+'}')
 location=read(SERVER/'core/game/world/map/Location.kt');nested=c.method(location,'value class LocationHash(');adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;fun transform(dx:Int,dy:Int,dz:Int)=Location(x+dx,y+dy,(z+dz)%4);'+c.method(location,'override fun equals(other: Any?)')+'\n'+next(line.strip()for line in location.splitlines()if'override fun toString()'in line)+'\noverride fun hashCode():Int=hash.hash;companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)}\n@JvmInline '+nested+'}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]));run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=180)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged Scenery, Constructed, SceneryDefinition, Definition, Pulse and ByteBufferUtils. Node base/interaction/attribute store, location/direction records and player varp/varbit/log adapters are explicit. Compares retained definition/name/size/child-array/location/attribute identity, live mutations, morph/wrapper selection, transforms/defaults, geometry, layers, equality and config writes. Cache decoding/attribute persistence have separate corpora. General mutable child-array/reflection APIs, cyclic user graphs, malformed recursive families, concrete interaction/player/world execution are outside this claim.')
 folder=ROOT/'fixtures/scenery-node'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Scenery node fixtures/provenance differ')
  print(f'{len(out)} original scenery-node scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original scenery-node scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
