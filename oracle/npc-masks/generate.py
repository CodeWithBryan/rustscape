"""The unchanged NPCFlags530 encoders over the original IoBuffer, with recording entity/skills adapters."""
import argparse, hashlib, importlib.util, itertools, json, os, pathlib, subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
RUN=ROOT/'runs/npc-masks-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py')
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
FLAGS=['PrimaryHit','SecondaryHit','Animate','FaceEntity','SpotAnim','TypeSwap','ForceChat','AnimationSequence','FaceLocation']
INTS=[-2147483648,-65536,-32769,-129,-1,0,1,127,128,255,256,32767,65535,65536,2147483647]
def npc(index=3,max=10,life=(4,4),kind='NPC'):return dict(type='Entity',**{'class':kind},index=index,max=max,life=list(life))
def hit(damage=5,kind=1,entity=None):return dict(type='HitMark',damage=damage,kind=kind,entity=entity)
BASE=[None,hit(entity=npc()),dict(type='Animation',id=808,delay=0),dict(type='Graphics',id=86,height=100,delay=0),
 npc(2,kind='Player'),npc(7),dict(type='Integer',value=42),dict(type='String',value='Moo'),dict(type='Location',x=3222,y=3218),dict(type='Long',value=5)]
def cases():
 rows=[dict(provider=f,context=x) for f,x in itertools.product(FLAGS,BASE)]
 for damage,kind in itertools.product(INTS,INTS):
  rows.append(dict(provider='PrimaryHit',context=hit(damage,kind,npc(life=(10,10)))))
  rows.append(dict(provider='SecondaryHit',context=hit(damage,kind,None)))
 lives=[(0,0),(5,5),(10,3),(3,9),(-1,-1),(11,11),(2147483647,2147483647),(8421505,8421505),(-2147483648,-2147483648)]
 for m,life,kind in itertools.product([0,1,10,99,-5,-1,2147483647],lives,['NPC','Player']):
  rows.append(dict(provider='PrimaryHit',context=hit(7,0,npc(max=m,life=life,kind=kind))))
 rows.append(dict(provider='PrimaryHit',context=hit(7,0,None)))
 for i,d in itertools.product(INTS,INTS):rows.append(dict(provider='Animate',context=dict(type='Animation',id=i,delay=d)))
 for kind,i in itertools.product(['Player','NPC'],INTS):rows.append(dict(provider='FaceEntity',context=npc(i,kind=kind)))
 for i,(h,d) in itertools.product(INTS,itertools.product([-1,0,1,100,65535],[-1,0,1,65535,65536])):rows.append(dict(provider='SpotAnim',context=dict(type='Graphics',id=i,height=h,delay=d)))
 for v in INTS:rows.append(dict(provider='TypeSwap',context=dict(type='Integer',value=v)))
 for s in ['','a','Moo','Quack!','Stand and deliver!','x'*200,'café ü','<col=ff0000>red']:rows.append(dict(provider='ForceChat',context=dict(type='String',value=s)))
 for x,y in itertools.product(INTS,INTS):rows.append(dict(provider='FaceLocation',context=dict(type='Location',x=x,y=y)))
 return rows
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 assert run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit']
 run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));reflect=next(p for p in jars if p.name.startswith('kotlin-reflect-'))
 pin=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=ROOT/'runs/legacy-interaction-oracle/json-simple-1.1.1.jar'
 assert hashlib.sha256(jar.read_bytes()).hexdigest()==pin['sha256']
 java=[];kt=[]
 def original(name):
  p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 # Unchanged: the encoders, their base class and flag enum, the context class
 # HitMark, and IoBuffer with the classes it compiles against.
 for name in ['core/game/world/update/flag/NPCFlags530.kt','core/game/world/update/flag/EFlagProvider.kt','core/game/world/update/flag/EntityFlag.kt',
              'core/game/world/update/flag/context/HitMark.java','core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java',
              'core/cache/crypto/ISAACCipher.kt','core/cache/misc/buffer/ByteBufferUtils.kt']:original(name)
 # Adapters: getters only (no field shares a Kotlin property's name), the
 # entity reads recorded in order.
 adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;
public class Entity {public core.game.node.entity.skill.Skills skillsValue;public int clientIndexValue;
public core.game.node.entity.skill.Skills getSkills(){return skillsValue;}
public int getClientIndex(){oracle.Support.emit("index");return clientIndexValue;}}
''')
 adapter('core/game/node/entity/player/Player.java','package core.game.node.entity.player; public class Player extends core.game.node.entity.Entity {}')
 adapter('core/game/node/entity/npc/NPC.java','package core.game.node.entity.npc; public class NPC extends core.game.node.entity.Entity {}')
 adapter('core/game/node/entity/skill/Skills.java','''package core.game.node.entity.skill;
public class Skills {public int maximumValue;public int[] lifeValues;int reads;
public int getMaximumLifepoints(){oracle.Support.emit("max");return maximumValue;}
public int getLifepoints(){int v=lifeValues[Math.min(reads,lifeValues.length-1)];reads++;oracle.Support.emit("life",v);return v;}}
''')
 adapter('core/game/world/update/flag/context/Animation.java','''package core.game.world.update.flag.context;
public class Animation {private final int i,d;public Animation(int id,int delay){i=id;d=delay;}public int getId(){return i;}public int getDelay(){return d;}}
''')
 adapter('core/game/world/update/flag/context/Graphics.java','''package core.game.world.update.flag.context;
public class Graphics {private final int i,h,d;public Graphics(int id,int height,int delay){i=id;h=height;d=delay;}public int getId(){return i;}public int getHeight(){return h;}public int getDelay(){return d;}}
''')
 adapter('core/game/world/map/Location.java','''package core.game.world.map;
public class Location {private final int a,b;public Location(int x,int y){a=x;b=y;}public int getX(){return a;}public int getY(){return b;}}
''')
 adapter('core/game/node/entity/impl/ForceMovement.java','package core.game.node.entity.impl; public class ForceMovement {}')
 adapter('core/api/Logging.kt','''package core.api
fun logWithStack(clazz:Class<*>,level:core.tools.Log,message:String){oracle.Support.emit("log",clazz.getSimpleName(),level.name,message)}
''')
 adapter('core/tools/Log.kt','package core.tools; enum class Log {ERR}')
 java=list(dict.fromkeys(java));kt=list(dict.fromkeys(kt))
 p=HERE/'Support.java';read(p);java.append(p);cp=os.pathsep.join(map(str,[classes,std,reflect,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',os.pathsep.join(map(str,[std,reflect,jar])),'-d',classes,*kt,*java])
 run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 rows_in=cases();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in rows_in),encoding='utf-8')
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=300)
 if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
 rows=[json.loads(x)for x in result.stdout.splitlines()if x];assert len(rows)==len(rows_in)
 assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,rows_in))
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',
  scope='Unchanged NPCFlags530 (all nine providers and writeTo bodies), EFlagProvider (logInvalidType), EntityFlag, HitMark, IoBuffer with PacketHeader, ISAACCipher and ByteBufferUtils on JDK 11 with -Dfile.encoding=UTF-8 (putString uses String.getBytes()). Entity/Player/NPC, Skills, Animation, Graphics, Location and ForceMovement are getter-only adapters; the entity reads (maximumLifepoints, lifepoints, getClientIndex) and logWithStack are recorded in order. Each case is one writeTo with its written bytes, recorded calls and NullPointerException/ArithmeticException. Not covered: the provider registry, UpdateMasks (update-masks oracle), the NPC renderer and buffers beyond 2048 bytes.')
 folder=ROOT/'fixtures/npc-masks';folder.mkdir(exist_ok=True)
 if check:assert (folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text())==provenance;print(f'{len(rows)} original NPCFlags530 cases reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n');print(f'Generated {len(rows)} original NPCFlags530 cases.')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
