"""Compile unchanged NPCDefinition/Definition/ByteBufferUtils with explicit services."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=pathlib.Path(os.environ.get('RS09_NPC_DEFINITION_ORACLE_RUN',ROOT/'runs/npc-definition-oracle'));SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
 out=[];rng=random.Random(530)
 def decode(b,**kw):out.append(dict(kind='decode',data=bytes(b).hex(),**kw))
 def seq(actions,initial=None,**kw):out.append(dict(kind='sequence',actions=actions,initial=initial or[],**kw))
 def op(kind,**kw):return dict(op=kind,**kw)
 commands=[[i]for i in [93,99,107,109,111]]
 for o in [12,100,101,119,125,128]:
  for v in [0,1,127,128,255]:commands.append([o,v])
 for o in [13,14,15,16,95,97,98,102,103,122,123,127,137]:
  for v in [0,1,32767,32768,65535]:commands.append([o,v>>8,v&255])
 for o in [2,30,31,32,33,34]:
  for text in [b'',b'Hidden',b'hidden',b'null',b'Attack',b'Examine',b'\x80\xff']:commands.append([o,*text,0])
 commands.extend([[1,0],[1,2,0,1,255,255],[17,0,1,128,0,255,255,127,255],[40,2,0,1,255,255,128,0,127,255],[41,1,128,0,255,255],[42,4,0,127,128,255],[60,2,0,1,255,255],[106,255,255,0,7,1,0,1,255,255],[118,0,9,255,255,0,2,1,0,1,255,255],[113,128,0,255,255],[114,128,255],[115,128,255],[121,2,1,128,0,255,2,127,255,0],[126,255,255,128,0],[134,0,1,128,0,255,255,0,2,128],[135,128,255,255],[136,1,0,2],[249,2,1,0,0,1,128,255,0,0,0,0,2,128,0,0,1]])
 decode([]);decode([0])
 for i in range(256):decode([i,255,128,1,0,0,0,0,0])
 for b in commands:
  decode(b+[0]);decode(b)
  for n in range(len(b)):decode(b[:n])
 for _ in range(100):
  b=sum(rng.choices(commands,k=rng.randrange(1,8)),[])+[0];decode(b);decode(b[:rng.randrange(len(b))])
 for value in [-2147483648,-1,0,1,2,3,2147483647]:
  for children in [None,[],[-1],[1,-1],[1,2,3],[-1,2,3],[0,1,-1]]:
   seq([op('child',slot=0,player=False,name='none'),op('child',slot=0,player=True,name='player')],[dict(slot=0,fields=dict(childNPCIds=children,configFileId=7,configId=11)),dict(slot=1,fields=dict(id=1)),dict(slot=2,fields=dict(id=2))],configValue=value)
 for name in [None,'','null','Null','Rat']:
  for options in [None,[],[None]*5,['Attack'],['attack'],['Hidden'],['Examine'],['null']]:seq([op('hasOptions',slot=0,examine=False),op('hasOptions',slot=0,examine=True),op('hasAttack',slot=0),op('hasAction',slot=0,value='attack'),op('hasAction',slot=0,value=None)],[dict(slot=0,fields=dict(name=name,options=options))])
 for id in [-2147483648,-1,0,1,128,8590,2147483647]:seq([op('load',files={str(id):'0c0200'}),op('lookup',id=id),op('retain',slot=id,name='old'),op('load',files={str(id):'0c0500'}),op('lookup',id=id)])
 for data in ['','01020001','01020001ffff00','7e0001800000']:
  seq([op('load',files={'7':data}),op('lookup',id=7),op('load',files={'7':'0c0400'}),op('lookup',id=7),op('parse',slot=7,data='01020003')])
 for field,value,change in [('options',['Attack',None],'Talk-to'),('childNPCIds',[1,-1],-1),('anIntArray868',[1,2],65535),('aShortArray859',[1],-32768),('aShortArray896',[1],32767),('aShortArray880',[1],-1),('aShortArray866',[1],0),('aByteArray861',[1],-128),('anIntArray892',[1],9)]:seq([op('retain',slot=0,name='before'),op('share',slot=0,to=1,field=field),op('alias',slot=1,to=3),op('array',slot=1,field=field,index=0,value=change),op('set',slot=0,fields={field:None})],[dict(slot=0,fields={field:value}),dict(slot=1,fields={})])
 for missing in [False,True]:seq([op('child',slot=0,player=True)],[dict(slot=0,fields=dict(childNPCIds=[1,-1],configFileId=7))],missingVarbit=missing)
 seq([op('config')])
 for key in ['id','movement_radius','lifepoints','melee_animation','clue_level','combat_style','protect_style','bonuses','combat_audio','slayer_exp','water_npc','name','unknown']:
  for value in [None,'','null','1',' 1','1,2','-1','2147483648',True,1.0,'NaN','Infinity']:
   row={'id':47,key:value};seq([op('config',text=json.dumps([row]))])
 for text in ['null','{}','[null]','[3]','[{}]','[,{"id":47 "water_npc":"true"},]','[']:
  seq([op('config',text=text)])
 for flag in ['TRUE','false','1',' true ','TrUe']:
  seq([op('config',text=json.dumps([dict(id=47,water_npc=flag,poison_immune=flag,facing_booth=flag)]))])
 for at in [1,2,3]:
  seq([op('config',text=json.dumps([dict(id=47,name='Rat',unknown='a',other='b')])),op('config',text=json.dumps([dict(id=47,name='',force_talk='hello')]))],failLog=at)
 seq([op('load',files={'47':'02014e5043000c0200'}),op('lookup',id=47),op('retain',slot=47,name='before'),op('config',text=json.dumps([dict(id=47,movement_radius='4',name='Configured',melee_animation='123',combat_style='2',clue_level='3')])),op('lookup',id=47)])
 for index in [-1,0,1,2,3,4,5,2147483647]:
  seq([op('config',text=json.dumps([dict(id=47,melee_animation='1',magic_animation='2',range_animation='3',defence_animation='4',death_animation='5')])),op('animation',slot=47,index=index)])
 for graphics in [None,[],[None],[None]*3]:
  for values in [{},{'start_gfx':1,'projectile':2,'end_gfx':3},{'start_gfx':None},{'start_gfx':'bad'},{'start_gfx':1,'projectile':None}]:
   seq([op('graphics',slot=0,values=values)],[dict(slot=0,fields=dict(combatGraphics=graphics,handlers={'start_height':-1,'prj_height':128,'end_height':255}))])
 seq([op('graphics',slot=0),op('graphics',slot=0,values={'projectile':7})],[dict(slot=0,fields=dict(handlers={'start_gfx':1,'projectile':2,'end_gfx':3}))])
 return out
def generate(check,audit_file=None):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
 run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 folder=ROOT/'fixtures/npc-definitions';folder.mkdir(parents=True,exist_ok=True);config=folder/'npc_configs.json'
 raw=run(['git','-C',ref,'show',lock['server']['commit']+':Server/data/configs/npc_configs.json']).stdout
 if check:
  if config.read_bytes()!=raw:raise RuntimeError('Pinned NPC configuration differs')
 else:config.write_bytes(raw)
 for name,expected in [('core/game/node/entity/combat/CombatStyle.java',['MELEE','RANGE','MAGIC']),('content/global/activity/ttrail/ClueLevel.java',['EASY','MEDIUM','HARD','UNKNOWN'])]:
  import re
  values=re.findall(r'^\s*([A-Z_]+)\(',read(SERVER/name),re.M)
  if values!=expected:raise RuntimeError('Enum order differs: '+name)
 cp=os.pathsep.join(map(str,[classes,std,jar]));java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 animator=read(SERVER/'core/game/node/entity/impl/Animator.java');strings=read(SERVER/'core/tools/StringUtils.java')
 adapters={
 'core/game/node/Node.java':'package core.game.node;public class Node{}',
 'core/game/node/entity/npc/NPC.java':'package core.game.node.entity.npc;public class NPC extends core.game.node.Node{}',
 'core/game/node/entity/npc/drop/NPCDropTables.java':'package core.game.node.entity.npc.drop;public class NPCDropTables{public NPCDropTables(core.cache.def.impl.NPCDefinition d){}}',
 'core/game/node/entity/player/Player.java':'package core.game.node.entity.player;public class Player{}',
 'core/game/interaction/OptionHandler.java':'package core.game.interaction;public class OptionHandler{}',
 'core/game/world/GameWorld.java':'package core.game.world;public class GameWorld{public static void prompt(boolean b){throw new UnsupportedOperationException();}}',
 'core/tools/Log.java':'package core.tools;public enum Log{WARN,FINE;}',
 'core/tools/SystemLogger.java':'package core.tools;public class SystemLogger{}',
 'core/ServerConstants.java':'package core;public class ServerConstants{public static String CONFIG_PATH;}',
 'core/tools/StringUtils.java':'package core.tools;public class StringUtils{'+c.method(strings,'public static boolean isPlusN(')+'}',
 'core/cache/def/impl/VarbitDefinition.java':'package core.cache.def.impl;public class VarbitDefinition{int value;VarbitDefinition(int v){value=v;}public static VarbitDefinition forNPCID(int id){Integer v=oracle.Support.varbit(id);return v==null?null:new VarbitDefinition(v);}public int getValue(core.game.node.entity.player.Player p){return value;}public int getVarpId(){throw new UnsupportedOperationException();}public int getStartBit(){throw new UnsupportedOperationException();}public int getEndBit(){throw new UnsupportedOperationException();}}',
 'core/cache/Cache.java':'package core.cache;public class Cache{public static Integer failFile;public static java.util.Map<Integer,byte[]>files=new java.util.HashMap<>();public static Index[]getIndexes(){Index[]a=new Index[19];a[18]=new Index();return a;}public static class Index{public byte[]getFileData(int group,int file){return oracle.Support.file((group<<7)|file);}}}',
 'core/game/world/map/Location.java':'package core.game.world.map;public class Location{}',
 'core/game/world/map/RegionManager.java':'package core.game.world.map;public class RegionManager{public static Chunk getRegionChunk(Location l){throw new UnsupportedOperationException();}public static class Chunk{public void flag(Object o){throw new UnsupportedOperationException();}}}',
 'core/game/world/update/flag/chunk/GraphicUpdateFlag.java':'package core.game.world.update.flag.chunk;public class GraphicUpdateFlag{public GraphicUpdateFlag(core.game.world.update.flag.context.Graphics g,core.game.world.map.Location l){throw new UnsupportedOperationException();}}',
 'core/game/node/entity/impl/Animator.java':'package core.game.node.entity.impl;public class Animator{'+c.method(animator,'public static enum Priority')+'}',
 'core/cache/def/impl/AnimationDefinition.java':'package core.cache.def.impl;public class AnimationDefinition{public static AnimationDefinition forId(int id){throw new UnsupportedOperationException();}public int getDurationTicks(){throw new UnsupportedOperationException();}}',
 'core/game/node/scenery/Scenery.java':'package core.game.node.scenery;public class Scenery{}',
 'core/game/node/entity/combat/CombatStyle.java':'package core.game.node.entity.combat;public enum CombatStyle{MELEE,RANGE,MAGIC;}',
 'content/global/activity/ttrail/ClueLevel.java':'package content.global.activity.ttrail;public enum ClueLevel{EASY,MEDIUM,HARD,UNKNOWN;}',
 'core/api/ContentAPI.kt':'@file:JvmName("ContentAPIKt")\npackage core.api\nfun log(c:Any?,l:core.tools.Log,s:String){oracle.Support.log(s)}\nfun getVarp(p:core.game.node.entity.player.Player,id:Int)=oracle.Support.varp(id)'
 }
 for name,text in adapters.items():adapter(name,text)
 for name in ['core/cache/def/Definition.java','core/cache/def/impl/NPCDefinition.java','core/cache/misc/buffer/ByteBufferUtils.kt','core/game/system/config/NPCConfigParser.kt','core/game/world/update/flag/context/Graphics.java','core/game/world/update/flag/context/Animation.java']:
  p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
 read(HERE/'Support.java');java.append(HERE/'Support.java')
 run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 def evaluate(inputs):
  p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in inputs),encoding='utf-8')
  with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Xmx3g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support',RUN/'config',config])),cwd=ROOT,stdin=f,capture_output=True,timeout=180)
  if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
  rows=[json.loads(l)for l in result.stdout.splitlines()if l];assert len(rows)==len(inputs);return rows
 rows=evaluate(requests());data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');read(ROOT/'tools/port_npc_schema.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),configSha256=hashlib.sha256(raw).hexdigest(),pinnedConfigRows=len(json.loads(raw)),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged NPCDefinition, Definition, ByteBufferUtils, Animation and Graphics compiled. Tests stored scalar/array/handler state excluding opaque dropTables service, raw decode/partial fields/positions, lazy cache identity and child/options queries; complete original NPCConfigParser conversions/field order with full pinned configuration, typed constructor values and partial loads. Explicit cache, varbit/player, NPC/drop-table/content/world services. No complete NPC class, drop tables, concrete animation execution or live world-startup claim.')
 folder=ROOT/'fixtures/npc-definitions';folder.mkdir(parents=True,exist_ok=True)
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('NPC definition fixtures/provenance differ')
  print(f'{len(rows)} original NPC-definition scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original NPC-definition scenarios.')
 if audit_file:
  p=RUN/'audit-output.json';p.write_text(json.dumps(evaluate([json.loads(audit_file.read_text(encoding='utf-8'))])[0],sort_keys=True,separators=(',',':'))+'\n',encoding='utf-8');print('Complete original NPC archive observed.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);p.add_argument('--audit-file',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check,a.audit_file)
