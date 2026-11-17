"""Compile unchanged server SceneryDefinition and compare stored fields/registry queries."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/scenery-definition-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def op(kind,**kw):return dict(op=kind,**kw)
def requests():
 rng=random.Random(530);out=[]
 def decode(data,id=530,**kw):out.append(dict(kind='decode',id=id,data=bytes(data).hex(),**kw))
 def sequence(actions,initial=None,**kw):out.append(dict(kind='sequence',initial=initial or[],actions=actions,**kw))
 commands=[[o]for o in [17,18,21,22,23,27,62,64,73,74,82,88,89,90,91,94,95,96,97]]
 for o in [14,15,19,28,29,39,69,75,81,101]:
  for v in [0,1,127,128,255]:commands.append([o,v])
 for o in [24,60,65,66,67,68,70,71,72,93,102]:
  for v in [0,1,32767,32768,65535]:commands.append([o,v>>8,v&255])
 for o in [2,30,31,32,33,34]:
  for text in [b'',b'Door',b'Hidden',b'hidden',b'Examine',b'null',b'Close',b'\x80\xff']:
   commands.append([o,*text,0])
 commands.extend([[1,0],[5,0],[1,1,0,7,10],[1,2,0,1,0,255,255,10],[5,2,0,1,255,255],[40,2,0,1,255,255,128,0,127,255],[41,1,128,0,255,255],[42,4,0,127,128,255],[78,255,255,128],[79,0,1,255,255,255,2,0,1,255,255],[100,128,255,255],[77,255,255,0,1,1,0,1,255,255],[92,0,7,255,255,0,9,1,0,1,255,255],[249,2,1,0,0,7,128,255,0,2,0,0,8,128,0,0,1]])
 decode([]);decode([0]);
 for o in range(256):decode([o,255,128,1,0,0,0,0,0])
 for command in commands:
  decode(command+[0]);decode(command)
  for end in range(len(command)):decode(command[:end])
 for id in [-2147483648,-1,0,29292,31017,2147483647]:
  decode([14,1,15,3,0],id);decode([74,19,0,77,0,7,255,255,0,0,1,0],id)
 for _ in range(150):
  data=sum(rng.choices(commands,k=rng.randrange(1,10)),[])+[0]
  decode(data,rng.choice([0,1,530,29292,31017]))
  decode(data[:rng.randrange(len(data))])
 for a,b in [(commands[-1],commands[-2]),([1,1,0,1,10],[5,2,0,7,0,8]),([5,1,0,3],[1,2,0,1,10,0,2,1]),([1,0],[1,1,0,1,10])]:decode(a+b+[0])
 decode([],failLog=1);decode([255,0],failLog=1)
 for index in [-2147483648,-2,-1,0,1,2,3,2147483647]:
  for children in [None,[],[-1],[1,-1],[1,2,3],[-1,2,3],[0,1,-1]]:
   initial=[dict(slot=0,fields=dict(id=0,name='Door',childrenIds=children,configFileId=7,configId=11)),dict(slot=1,fields=dict(id=1,name='Door',options=['Open',None,None,None,None])),dict(slot=2,fields=dict(id=2,name='null',interactable=1))]
   sequence([op('retain',slot=0,name='parent'),op('childIndex',slot=0,index=index,name='index'),op('child',slot=0,player=False,name='none'),op('child',slot=0,player=True,name='player'),op('hasActions',slot=0)],initial,configValue=index)
 for name in [None,'','null','Null','Door','Examine']:
  for options in [None,[],[None]*5,['Examine'],['examine'],['null'],['NULL'],['Open',None,None,None,None]]:
   sequence([op('hasOptions',slot=0,examine=False),op('hasOptions',slot=0,examine=True),op('hasActions',slot=0),op('hasAction',slot=0,value='open'),op('hasAction',slot=0,value=None),op('configure',slot=0)], [dict(slot=0,fields=dict(name=name,options=options))])
 for children in [[-1,2,0],[2,3,-1],[0],[],None]:
  files={'0':bytes([92,0,7,255,255,0,2,0,0,1,0]).hex(),'1':bytes([0]).hex(),'2':bytes([92,0,9,255,255,255,255,0,0,3,0]).hex()}
  sequence([op('retain',slot=1,name='old'),op('configure',slot=0),op('retain',slot=2,name='child'),op('load',count=4,files=files),op('hasActions',ref='old'),op('hasActions',slot=-1)], [dict(slot=0,fields=dict(id=0,childrenIds=children,configFileId=55))])
 for failed in [False,True]:
  sequence([op('retain',slot=0,name='old'),op('load',count=3,files={'0':'00','1':'0101','2':'00'}),op('container',id=-1)],failLog=1 if failed else -1)
 for field,value,changed in [('options',['Open',None],'Close'),('childrenIds',[1,2],-1),('modelIds',[1,2],65535),('modelConfiguration',[10],0),('originalColors',[1],-32768),('modifiedColors',[1],32767),('aShortArray3919',[1],-1),('aShortArray3920',[1],0),('aByteArray3858',[1],-128),('anIntArray3859',[1],9)]:
  sequence([op('retain',slot=0,name='before'),op('share',slot=0,to=1,field=field),op('alias',slot=1,to=3),op('array',slot=1,field=field,index=0,value=changed),op('set',slot=0,fields={field:None}),op('load',count=2,files={}),op('hasActions',ref='before')],[dict(slot=0,fields={field:value}),dict(slot=1,fields={})])
 for kw in [dict(failVarbit=True),dict(missingVarbit=True),dict(failVarp=True)]:
  sequence([op('child',slot=0,player=True,name='child')],[dict(slot=0,fields=dict(childrenIds=[1,2],configFileId=-1 if'failVarp'in kw else 7,configId=9))],**kw)
 samples=json.loads(read(HERE/'cache-samples.json'))if(HERE/'cache-samples.json').is_file()else{}
 for id,data in samples.items():out.append(dict(kind='decode',id=int(id),data=data))
 return out
def generate(check,audit_file):
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
 cp=os.pathsep.join(map(str,[classes,std,jar]));java=[]
 utils=SERVER/'core/cache/misc/buffer/ByteBufferUtils.kt';read(utils);run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,utils])
 cache=read(SERVER/'core/cache/Cache.java');strings=read(SERVER/'core/tools/StringUtils.java')
 adapters={
 'core/game/node/Node.java':'package core.game.node;public class Node{}',
 'core/game/node/scenery/Scenery.java':'package core.game.node.scenery;public class Scenery extends core.game.node.Node{}',
 'core/game/node/entity/player/Player.java':'package core.game.node.entity.player;public class Player{}',
 'core/game/interaction/OptionHandler.java':'package core.game.interaction;public class OptionHandler{}',
 'core/game/world/GameWorld.java':'package core.game.world;public class GameWorld{public static void prompt(boolean b){throw new UnsupportedOperationException();}}',
 'core/tools/Log.java':'package core.tools;public class Log{public static int ERR=1;}',
 'core/tools/StringUtils.java':'package core.tools;public class StringUtils{'+c.method(strings,'public static boolean isPlusN(')+'}',
 'core/api/ContentAPIKt.java':'package core.api;public class ContentAPIKt{public static void log(Object c,int l,String s){oracle.Support.log(s);}public static int getVarp(core.game.node.entity.player.Player p,int id){return oracle.Support.varp(id);}}',
 'core/cache/def/impl/VarbitDefinition.java':'package core.cache.def.impl;public class VarbitDefinition{int value;VarbitDefinition(int v){value=v;}public static VarbitDefinition forObjectID(int id){Integer v=oracle.Support.varbit(id);return v==null?null:new VarbitDefinition(v);}public int getValue(core.game.node.entity.player.Player p){return value;}}',
 'core/cache/Cache.java':'package core.cache;public class Cache{public static int count;public static Integer failFile;public static java.util.Map<Integer,byte[]>files=new java.util.HashMap<>();public static Index[]getIndexes(){Index[]a=new Index[17];a[16]=new Index();return a;}'+c.method(cache,'public static final int getObjectDefinitionsSize()')+'public static class Index{public int getContainersSize(){return(count+255)/256;}public int getFilesSize(int last){return count-last*256;}public byte[]getFileData(int group,int file){int id=group*256+file;if(failFile!=null&&id==failFile)throw new oracle.Support.Failure();return files.get(id);}}}'
 }
 for name,text in adapters.items():
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 for path in [SERVER/'core/cache/def/Definition.java',SERVER/'core/cache/def/impl/SceneryDefinition.java',HERE/'Support.java']:read(path);java.append(path)
 run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 def evaluate(inputs):
  file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in inputs),encoding='utf-8')
  large=audit_file is not None and len(inputs)==1
  options=['-Xmx8g','-Xlog:gc*:file=runs/scenery-definition-oracle/audit-gc.log:time,level']if large else['-Xmx2g']
  with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,*options,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=600 if large else 180)
  if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
  out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(inputs);return out
 out=evaluate(requests());data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');read(ROOT/'tools/port_scenery_schema.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged SceneryDefinition, Definition and ByteBufferUtils. Explicit cache file/count, logging and player/varbit value adapters. Native decoder, registry load/lookup/configuration, child selection and action/options queries compare all stored fields and array/map/definition aliases, logs, positions and partial failures. Unported base examine text, option-handler invocation/default map, varbit cache parsing, content configurations and world integration are not established by this corpus. Schema authoring is offline; runtime uses Rust only.')
 folder=ROOT/'fixtures/scenery-definitions'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Scenery definition fixtures/provenance differ')
  print(f'{len(out)} original scenery-definition scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original scenery-definition scenarios.')
 if audit_file:
  result=evaluate([json.loads(audit_file.read_text(encoding='utf-8'))])[0];(RUN/'audit-output.json').write_text(json.dumps(result,sort_keys=True,separators=(',',':'))+'\n',encoding='utf-8');print('Original full scenery archive observed.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);p.add_argument('--audit-file',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check,a.audit_file)
