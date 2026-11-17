"""Complete original UpdateMasks with a controlled clock and explicit native-boundary services."""
import argparse, hashlib, importlib.util, itertools, json, os, pathlib, re, subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
RUN=ROOT/'runs/update-masks-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
FLAGS=['Chat','ForceChat','PrimaryHit','SecondaryHit','Animate','SpotAnim','Appearance','FaceEntity','FaceLocation','ForceMove','AnimSeq','TypeSwap']
def op(kind,**kw):return dict(op=kind,**kw)
def register(flag='FaceLocation',ctx=0,sync=False):return op('register',flag=flag,ctx=ctx,sync=sync)
def requests():
 rows=[]
 for kind,flag,sync in itertools.product(['Player','NPC'],FLAGS,[False,True]):
  rows.append(dict(kind=kind,actions=[register(flag,sync=sync),op('write'),op('write',synced=True,appearance=False),op('reset'),op('write',synced=True),op('unregister',ordinal=10),op('write',synced=True)]))
 for presence,ordinal,sync in itertools.product([-2147483648,-1,0,1,255,256,0x10000,2147483647],[-1,0,3,10,11],[False,True]):
  rows.append(dict(providers=[dict(flag='Appearance',presence=presence,ordinal=ordinal)],actions=[register('Appearance',None,sync),op('write'),op('reset'),op('write',synced=True),op('unregister',ordinal=ordinal)]))
 for entity,viewer,synced,appearance in itertools.product([None,0,1],[None,0],[False,True],[False,True]):
  rows.append(dict(actions=[register('FaceLocation',sync=True),register('SpotAnim'),op('write',entity=entity,viewer=viewer,synced=synced,appearance=appearance)]))
 rows += [dict(actions=[register('FaceLocation',sync=True),op('context',id=0,value=42),op('reset'),op('write',synced=True),op('unregister',ordinal=10),op('write',synced=True)]),
          dict(actions=[register('FaceLocation',sync=True),op('unregister',ordinal=10),op('write'),op('reset'),op('write',synced=True)]),
          dict(providers=[dict(flag='FaceLocation',presence=4,ordinal=3,encodedFlag='FaceLocation')],actions=[register('Appearance',sync=True),register('FaceLocation',ctx=1),op('reset'),op('write',synced=True,appearance=False),op('unregister',ordinal=3)]),
          dict(actions=[op('stamp',value=-9223372036854775808),op('prepare'),register('Appearance'),op('reset'),register('Appearance')])]
 for encoded,ordinal in itertools.product(FLAGS,[0,3,10]):
  rows.append(dict(providers=[dict(flag='FaceLocation',presence=64,ordinal=ordinal,encodedFlag=encoded)],actions=[register('Appearance'),register('FaceLocation',sync=True),op('write',synced=True,appearance=False),op('reset'),op('write',synced=True,appearance=False),op('unregister',ordinal=ordinal),op('write',synced=True)]))
 for synced,appearance in itertools.product([False,True],[False,True]):
  rows.append(dict(hooks=[dict(event='encode',at=1,actions=[op('reset'),register('Animate',ctx=1),op('write',synced=synced,appearance=appearance)])],actions=[register('FaceLocation',sync=True),register('Appearance'),op('write',synced=True),op('write',synced=True,appearance=False)]))
 mutations=[op('fail'),op('reset'),register('FaceLocation',ctx=1,sync=True),register('Appearance',ctx=1),
  op('context',id=0,value=-99),op('unregister',ordinal=10),op('handler',entity=0,value=1),
  op('queue',handler=0,value=[]),op('queue',handler=1,value=[[7,2]]),
  op('provider',flag='FaceLocation',value=None),op('provider',flag='Appearance',value=dict(presence=0x200,ordinal=7)),op('prepare')]
 for event,at,change in itertools.product(['provider','clock','presence','encode','handler','peek','poll','missing'],[1,2],mutations):
  rows.append(dict(queue=[[2,1],[3,2],[4,0]],hooks=[dict(event=event,at=at,actions=[change])],actions=[register('FaceLocation',sync=True),register('Appearance'),op('write',synced=True,appearance=False),op('prepare'),register('TypeSwap'),op('write'),op('reset'),op('write',synced=True)]))
 for queue,entity in itertools.product([[],[[1,0]],[[1,0],[2,1]],[[1,0],[2,1],[3,2]]],[0,1]):
  rows.append(dict(queue=queue,actions=[op('prepare',entity=entity),op('prepare',entity=entity),register('FaceLocation'),op('write'),op('reset'),op('prepare',entity=entity),op('write')]))
 return rows
def generate(check,folder=None,scope=None,extras=None):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 assert run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit']
 run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));reflect=next(p for p in jars if p.name.startswith('kotlin-reflect-'))
 pin=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=ROOT/'runs/legacy-interaction-oracle/json-simple-1.1.1.jar'
 assert hashlib.sha256(jar.read_bytes()).hexdigest()==pin['sha256']
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 source=read(SERVER/'core/game/world/update/UpdateMasks.kt')
 assert source.count('System.currentTimeMillis()')==1
 adapter('core/game/world/update/UpdateMasks.kt',source.replace('System.currentTimeMillis()','oracle.Support.now()'))
 for name in ['flag/EntityFlag.kt','flag/EFlagProvider.kt','flag/context/HitMark.java']:
  p=SERVER/'core/game/world/update'/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
 # The table is extracted from the original constructor arguments, not copied from Rust.
 table={}
 for kind,parent in [('Player','PlayerFlags530'),('NPC','NPCFlags530')]:
  source=read(SERVER/f'core/game/world/update/flag/{parent}.kt')
  table[kind]=[dict(flag=f,presence=int(p,0),ordinal=int(o)) for p,o,f in re.findall(r':\s*'+parent+r'\s*\(\s*(0x[0-9a-fA-F]+|\d+)\s*,\s*(\d+)\s*,\s*EntityFlag\.(\w+)\s*\)',source)]
  assert table[kind]
 adapter('core/game/world/update/flag/EntityFlags.kt','''package core.game.world.update.flag
object EntityFlags {
@JvmStatic fun getFlagFor(type:EFlagType,flag:EntityFlag):EFlagProvider?=oracle.Support.provider(type,flag)
@JvmStatic fun getPresenceFlag(type:EFlagType,flag:EntityFlag):Int=oracle.Support.presence(type,flag)
}
''')
 adapter('core/game/world/update/flag/UpdateFlag.java','package core.game.world.update.flag; public abstract class UpdateFlag {}')
 adapter('core/api/Logging.kt','''package core.api
fun logWithStack(clazz:Class<*>,level:core.tools.Log,message:String){oracle.Support.missing(message)}
''')
 adapter('core/tools/Log.kt','package core.tools; enum class Log {ERR}')
 adapter('core/net/packet/IoBuffer.java','''package core.net.packet; public class IoBuffer {
public java.io.ByteArrayOutputStream out=new java.io.ByteArrayOutputStream();
public IoBuffer put(int v){out.write(v);return this;}}''')
 adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;
public class Entity {public int id,handler;public Entity(int i){id=i;}
public core.game.node.entity.combat.ImpactHandler getImpactHandler(){core.game.node.entity.combat.ImpactHandler h=oracle.Support.handlers[handler];oracle.Support.emit("handler",id,handler);return h;}}
''')
 adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player; public class Player extends core.game.node.entity.Entity {public Player(int id){super(id);}}')
 adapter('core/game/node/entity/combat/ImpactHandler.java','''package core.game.node.entity.combat;
public class ImpactHandler {public enum Type{A,B,C};public static class Impact {public int amount;public Type type;public Impact(int a,int t){amount=a;type=Type.values()[t];}}
public static class Queue extends java.util.LinkedList<Impact>{public int id;public Queue(int i){id=i;}
@Override public Impact peek(){Impact i=super.peek();oracle.Support.emit("peek",id,i==null?null:java.util.Arrays.asList(i.amount,i.type.ordinal()));return i;}
@Override public Impact poll(){Impact i=super.poll();oracle.Support.emit("poll",id,i==null?null:java.util.Arrays.asList(i.amount,i.type.ordinal()));return i;}}
public Queue impactQueue;public ImpactHandler(int i){impactQueue=new Queue(i);}public Queue getImpactQueue(){return impactQueue;}}
''')
 if extras:extras(adapter,java,kt)
 java=list(dict.fromkeys(java));kt=list(dict.fromkeys(kt))
 p=HERE/'Support.java';read(p);java.append(p);cp=os.pathsep.join(map(str,[classes,std,reflect,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',os.pathsep.join(map(str,[std,reflect,jar])),'-d',classes,*kt,*java])
 run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=[dict(case,defaults=table) for case in requests()];p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8')
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
 rows=[json.loads(x)for x in result.stdout.splitlines()if x];assert len(rows)==len(cases)
 assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete UpdateMasks with sole currentTimeMillis call redirected to controlled clock; complete EntityFlag, EFlagProvider and HitMark. Original provider constructor table extracted from PlayerFlags530/NPCFlags530. Provider registry, byte-recording encoders, entity/impact queues and logger are explicit reentrant services. Header bytes, retained contexts/providers, current versus persistent slots, prepare and failed write prefixes are compared. This does not establish concrete flag encoder, impact/combat, live Actor/renderer or full registry startup parity.')
 if scope:provenance['scope']=scope
 folder=folder or ROOT/'fixtures/update-masks';folder.mkdir(exist_ok=True)
 if check:assert (folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text())==provenance;print(f'{len(rows)} original update-mask scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n');print(f'Generated {len(rows)} original update-mask scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
