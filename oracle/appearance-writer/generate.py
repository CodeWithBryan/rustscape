"""Original late appearance writer with complete original Appearance and IoBuffer."""
import argparse,copy,hashlib,importlib.util,json,os,pathlib,random,subprocess
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).resolve().parent;O=R/'runs/appearance-writer-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('appearance',R/'oracle/player-appearance/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);c=base.c

spec=importlib.util.spec_from_file_location('legacy',R/'oracle/appearance/generate.py');legacy=importlib.util.module_from_spec(spec);spec.loader.exec_module(legacy)

def requests():
 rows=[];template=base.requests()[0]
 def add(wire=None,hooks=None,writes=None,**kw):
  row=copy.deepcopy(template);row['items'].update(weapon=dict(id=1),chest=dict(id=2),hat=dict(id=3));row.update(actions=[],equipment={},hooks=[],wire=wire or{},wireHooks=hooks or[],writes=writes or[{}],prefix=[170,85]);row.update(kw);rows.append(row)
 for pvp in [False,True]:
  for wild in [False,True]:
   for wilderness in [False,True]:
    for disabled in [False,True]:
     for using in [False,True]:add(dict(pvp=pvp,wild=wild,wilderness=wilderness,disabled=disabled,using=using))
 for size in [-2147483648,-1,0,1,2,8,33,2147483647]:
  for npc in [-1,1]:
   for gender in [0,1]:add(dict(size=size,npc=npc,gender=gender))
 for text in [None, '', 'Zezima', 'a_b__', '0123456789012345', '\xc3\xa2\xcb\u0153\xc6\u2019Rune', '\xc3\xb0\xc5\xb8\xcb\u0153\xe2\u201a\xacabc', '\xc3\u201e\xc2\xb0I', '_____']:
  add(dict(username=text))
 for i in range(7):add(dict(prepared=True,nullPart=i))
 add(dict(gender=None));add(dict(prepared=True,body=[-1,0,255,256,32768,65535,65536,2147483647,-2147483648,1,2,3,4,5]))
 for slot in [0,1,2,3,4,5,7,9,10]:add(equipment={str(slot):'weapon'})
 for redirect in [False,True]:
  for own in [False,True]:add(dict(colors=[1,2,3,4,5,6,7]),redirect=redirect,nullOwner=own)
 for invalid in ['null','String','Integer']:
  for null in [False,True]:add(writes=[dict(invalid=invalid,nullBuffer=null)])
 add(writes=[dict(nullBuffer=True)])
 keys=['appearance','render','prepared','setPrepared','equipment','skull','wilderness','disabled','bodySize','attribute','username','world','pvp','wild','properties','current','familiar','using','combatLevel','summoning','level','skills','total','invalidType']
 for mode in [{},{'pvp':True},{'wilderness':True,'disabled':True},{'wild':True,'wilderness':True}]:
  for key in keys:
   for at in [1,2]:add(mode,[dict(when=key,at=at,action='fail')],writes=[{},{}])
 for key in ['skull','wilderness','disabled','world','pvp','wild','properties','current','combatLevel','familiar','using','summoning','skills','total']:
  family='world'if key in ['world','pvp','wild']else'skull'if key in ['skull','wilderness','disabled']else'properties'if key in ['properties','current','combatLevel']else'skills'if key in ['skills','total']else'familiar'
  for value in [-1,1]:add(dict(pvp=True,wilderness=True),[dict(when=key,action='active',family=family,value=value)],writes=[{},{}])
 for key in ['appearance','equipment','bodySize','attribute','username','world','properties','current']:
  for action in [dict(action='redirect'),dict(action='size',value=7),dict(action='color',index=0,value=99),dict(action='override',value=None),dict(action='override',value='bad'),dict(action='buffer',value=77),dict(action='position',value=0),dict(action='reenter')]:
   add(dict(pvp=True),[dict(when=key,**action)])
 for key in ['appearance','bodySize','attribute','username']:
  add({},[dict(when=key,action='nullAppearance')],writes=[{},{}])
 for field,family in [('combat','properties'),('current','properties'),('wilderness','skull'),('using','familiar'),('summoning','familiar'),('pvp','world'),('wild','world'),('total','skills')]:
  for key in ['properties','current','combatLevel','world','pvp','username']:
   add(dict(pvp=True),[dict(when=key,action='set',family=family,field=field,value=17 if field in ['combat','current','summoning','total']else False)])
 rng=random.Random(530)
 for _ in range(30):add(dict(size=rng.randrange(-1,10),pvp=bool(rng.randrange(2)),wild=bool(rng.randrange(2)),prepared=True,body=[rng.randrange(-100000,100000)for _ in range(14)],colors=[rng.randrange(-1000,1000)for _ in range(7)],combat=rng.randrange(-2**31,2**31),current=rng.randrange(-2**31,2**31),summoning=rng.randrange(-2**31,2**31)))
 for key in ['appearance','attribute','username']:
  add(dict(pvp=True),[dict(when=key,action='redirect')],writes=[{},dict(player=1),{}])
 add(dict(size=2),[dict(when='bodySize',at=2,action='size',value=0)])
 add(dict(pvp=True),writes=[dict(player=1),{},dict(player=1)])
 add({},[dict(when='username',action='position',value=0)],prefix=[9]*300,writes=[{},{}])
 for old in [json.loads(line)for line in(R/'fixtures/appearance/oracle.jsonl').open(encoding='utf-8')]:
  if old.get('kind','appearance')!='appearance':continue
  w=dict(pvp=old.get('pvp',False),wild=old.get('wildPvp',False),wilderness=old.get('wilderness',False),disabled=old.get('wildernessDisabled',False),using=old.get('usingSummoning',False),combat=old.get('combatLevel',3),current=old.get('currentCombatLevel',3),summoning=old.get('summoningCombatLevel',0),total=old.get('totalLevel',33),level=old.get('wildernessLevel',0),username=old.get('username','self'),size=old.get('size',1),npc=old.get('npcId',-1),gender=old.get('gender',0),render=old.get('renderAnimation',1426),skull=old.get('skullIcon',-1),head=old.get('headIcon',-1))
  colors=[0]*7
  for index,value in zip([0,2,5,6,4],old.get('colors',[0]*5)):colors[index]=value
  w['colors']=colors
  if 'bodyParts'in old:w.update(prepared=True,body=old['bodyParts']+[0,0])
  add(w,legacyExpected=old['expected'])
 for text in ['\u2603Rune','\U0001f600abc','\u0130I']:add(dict(username=text))
 return rows

def generate(check):
 base.generate(True);legacy.c.JAVA_HOME=c.JAVA_HOME;legacy.generate(True);c.SOURCES.update(legacy.c.SOURCES);c.read(R/'fixtures/appearance/oracle.jsonl');assert 'private int level;'in c.read(S/'core/game/node/entity/player/link/SkullManager.java');assert 'private final Appearance appearance'in c.read(S/'core/game/node/entity/player/Player.java');assert 'private final Properties properties'in c.read(S/'core/game/node/entity/Entity.java');O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[];kt=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 source=c.read(R/'oracle/player-appearance/Player.java').replace('AppearanceOracle.hit(', 'oracle.WriterOracle.hit(')
 source=source.replace('public Player(int id){this.id=id;}', 'public final Skull[] skulls={new Skull(0),new Skull(1)};public final Familiar[] familiars={new Familiar(0),new Familiar(1)};public final Skills[] skillSlots={new Skills(0),new Skills(1)};public final Properties[] propSlots={properties,new Properties()};public int activeSkull,activeFamiliar,activeSkills,activeProp;public String username="self";public Player(int id){this.id=id;propSlots[0].ref=0;propSlots[1].ref=1;}public int size(){oracle.WriterOracle.hit("bodySize",id);return size;}public String getUsername(){oracle.WriterOracle.hit("username",id);return username;}public Skull getSkullManager(){oracle.WriterOracle.hit("skull",id);return activeSkull<0?null:skulls[activeSkull];}public Familiar getFamiliarManager(){oracle.WriterOracle.hit("familiar",id);return activeFamiliar<0?null:familiars[activeFamiliar];}public Skills getSkills(){oracle.WriterOracle.hit("skills",id);return activeSkills<0?null:skillSlots[activeSkills];}')
 source=source.replace('return properties;}','return activeProp<0?null:propSlots[activeProp];}')
 source=source.replace('public static class Properties{','public static class Properties{public int ref,combat=51,current=57;public int getCombatLevel(){oracle.WriterOracle.hit("combatLevel",ref);return combat;}public int getCurrentCombatLevel(){oracle.WriterOracle.hit("current",ref);return current;}')
 source=source[:-2]+c.read(H/'receivers.java.inc')+'\n}\n'
 source=source.replace('Appearance appearance;', 'Appearance appearanceField;').replace('return appearance;', 'return appearanceField;').replace('Properties properties=', 'Properties propertiesField=').replace('{properties,new Properties()}', '{propertiesField,new Properties()}').replace('String username=', 'String usernameField=').replace('return username;', 'return usernameField;')
 adapter('core/game/node/entity/player/Player.java',source)
 observer=c.read(R/'oracle/player-appearance/AppearanceOracle.java')
 import re
 observer=re.sub(r'\.appearance\b(?!\.)','.appearanceField',observer);observer=re.sub(r'\.properties\b','.propertiesField',observer)
 adapter('oracle/AppearanceOracle.java',observer)
 adapter('core/game/world/GameWorld.kt',c.read(H/'GameWorld.kt'))
 flags=c.read(S/'core/game/world/update/flag/PlayerFlags530.kt');provider=c.read(S/'core/game/world/update/flag/EFlagProvider.kt')
 adapter('core/game/world/update/flag/Appearance530.kt','''package core.game.world.update.flag
import core.net.packet.IoBuffer
import core.game.node.entity.player.Player
import core.game.world.GameWorld
import core.tools.StringUtils
import core.tools.Log
import core.api.logWithStack
import kotlin.reflect.*
enum class EntityFlag { Appearance }
open class PlayerFlags530(p:Int,o:Int,f:EntityFlag) {
open fun writeTo(buffer:IoBuffer,context:Any?) {}
'''+c.method(provider,'fun logInvalidType')+'\n'+c.method(flags,'class Appearance : PlayerFlags530')+'\n}\n')
 adapter('core/tools/Log.java','package core.tools;public enum Log{ERR;}')
 adapter('core/api/WriterLog.kt','@file:JvmName("WriterLog")\npackage core.api\nfun logWithStack(c:Class<*>,l:core.tools.Log,m:String){oracle.WriterOracle.logged(m)}')
 strings=c.read(S/'core/tools/StringUtils.java');adapter('core/tools/StringUtils.java','package core.tools;public class StringUtils{'+c.method(strings,'public static long stringToLong(')+'}')
 for name in ['core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java']:
  p=S/name;c.read(p);java.append(p)
 for name in ['core/cache/crypto/ISAACCipher.java','core/cache/misc/buffer/ByteBufferUtils.java']:adapter(name,c.read(R/'oracle/scenery/adapters'/name))
 p=H/'WriterOracle.java';c.read(p);java.append(p)
 jars=c.artifacts();cp=os.pathsep.join(map(str,[classes,R/'runs/player-appearance-oracle/classes',R/'runs/inventory-oracle/classes',R/'runs/animator-oracle/classes',R/'runs/inventory-oracle/json-simple-1.1.1.jar',*jars]))
 c.run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 data=''.join(json.dumps(row)+'\n'for row in requests()).encode();(O/'input.jsonl').write_bytes(data);exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 result=subprocess.run([str(exe),'-Dfile.encoding=UTF-8','-cp',cp,'oracle.WriterOracle'],cwd=R,input=data,capture_output=True,timeout=180);(O/'stderr.log').write_bytes(result.stderr);assert result.returncode==0,result.stderr.decode(errors='replace')
 rows=[json.loads(line)for line in result.stdout.splitlines()if line];assert len(rows)==len(requests());assert {'appearance','username','properties'}<=set(e[0]for e in rows[0]['expected']['trace']), 'Kotlin must bind original Player getters, never public observer fields';assert 'level'in[e[0]for e in rows[16]['expected']['trace']], 'Kotlin must call original private skull level getter';assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,requests()))
 assert all(j['expected']['results'][0]['bytes'][4:]==j['legacyExpected']for j in rows if 'legacyExpected'in j),'Full-class writer differs from legacy original field bytes'
 data=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();c.read(pathlib.Path(__file__));c.read(R/'fixtures/appearance-writer/baseline-468.json')
 provenance=dict(format=1,upstream=json.loads(c.read(R/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),scope='Verbatim PlayerFlags530.Appearance and EFlagProvider.logInvalidType, complete original Appearance/BodyPart/Gender/Container/Item/Animation/IoBuffer, and original name encoding. Required player Properties/skull/familiar/skills/world settings are recording receivers. Compares late lookups, receiver capture/replacement, equipped body mutation, nullable services, invalid contexts, original Kotlin null-buffer check, shared output cursor/reentry and partial writes. Full provider registry, Properties and player service/gameplay bindings remain open.')
 folder=R/'fixtures/appearance-writer';folder.mkdir(exist_ok=True)
 baseline=json.loads(c.read(folder/'baseline-468.json'));assert hashlib.sha256((''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows[:468])).encode()).hexdigest()==baseline['sha256'],'Prior468 writer observations changed'
 if check:assert(folder/'oracle.jsonl').read_bytes()==data and json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
 print(len(rows),'original late appearance writer scenarios reproduced'if check else'original late appearance writer scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
