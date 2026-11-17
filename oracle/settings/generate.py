"""Execute the complete unchanged pinned Settings class with owning-player adapters."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/settings-oracle'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
read,run=c.read,c.run

def requests():
 cases=[]
 def a(op,**kw):return dict(op=op,**kw)
 def add(actions,**config):cases.append(dict(actions=actions,config=config))
 setters=['brightness','music','sound','area','public','private','trade','clan','assist','attackToggle','attackSet','specialEnergy']
 for op in setters:
  for value in [-2147483648,-65537,-1,0,1,3,255,65536,2147483647]:add([a(op,value=value),a('update')])
 add([a(op)for op in ['update','specialToggle','specialToggle','retaliate','mouse','effects','split','aid','acceptAid','runToggle','weight','chat']])
 for ironman in [False,True]:
  for aid in [False,True]:add([a('state',fields=dict(acceptAid=aid)),a('acceptAid'),a('update')],ironman=ironman)
 for public in [-1,0,1,300]:
  for private in [-1,0,2,256]:
   for trade in [-1,0,3]:add([a('setChat',public=public,private=private,trade=trade),a('chatValues',public=public,private=99,trade=trade),a('chatValues',public=public+1,private=999,trade=trade-1),a('chat')])
 for initial in [False,True]:
  for energy in [-2147483648,-1,0,1,50,100,2147483647]:
   for amount in [-2147483648,-1,0,1,100,101,2147483647]:add([a('state',fields=dict(specialToggled=initial,specialEnergy=energy)),a('specialDrain',value=amount),a('specialSet',value=False),a('specialSet',value=False)])
 floats=['-Infinity','-1e300','-100','-0.0','0','0.125','99.5','100','101','1e300','Infinity','NaN']
 for energy in floats:
  for drain in floats:add([a('energySet',value=energy),a('energy',value=drain),a('update')])
 defaults=dict(brightness=1,musicVolume=2,soundEffectVolume=3,areaSoundVolume=4,singleMouse=True,disableChatEffects=True,splitPrivate=True,acceptAid=True,runToggled=True,publicChatSetting=5,privateChatSetting=6,clanChatSetting=7,tradeSetting=8,assistSetting=9,runEnergy='88.25',specialEnergy=72,attackStyle=2,retaliation=True)
 add([a('parse',data=defaults),a('update')])
 for key in defaults:
  for value in [None,'',True,False,3.0,' 3','-2147483649','2147483648','NaN','Infinity',[],{}]:
   row=dict(defaults);row[key]=value;add([a('parse',data=row),a('update')])
  row=dict(defaults);del row[key];add([a('parse',data=row)])
 for data in [None,[],3,'bad']:add([a('parse',data=data)])
 def weight_actions(inventory,equipment):return[a('slots',id=0,slots=inventory),a('slots',id=1,slots=equipment),a('weight')]
 for value in floats:
  for kind in ['double','long','raw']:
   if kind=='long':v=3
   elif kind=='raw':v='bad'
   else:v=value
   add(weight_actions([1,1,None]+[None]*25,[1,1]+[None]*12),weights={'1':dict(kind=kind,value=v)})
 add(weight_actions([1]+[None]*27,[2]+[None]*13),weights={'1':dict(value='-4.5'),'2':dict(value='-2.25')})
 add(weight_actions([1]+[None]*27,[None]*11+[2,2,2]),weights={'1':dict(value='2.5'),'2':dict(value='100')})
 add(weight_actions([1]+[None]*27,[None]*14),weights={'1':None})
 for length in [0,1,10,11,13,14,27,28,29]:add(weight_actions([None]*length,[None]*length))
 events=['varp','properties','retaliating','setRetaliating','dispatcher','energy','session','send','ironmanManager','ironman','inventory','equipment','item','definition','weight','string','message']
 for event in events:
  for nth in [1,2]:
   actions=[a('update'),a('slots',id=0,slots=[1]+[None]*27),a('slots',id=1,slots=[1]+[None]*13),a('weight'),a('acceptAid'),a('retaliate'),a('state',fields=dict(specialToggled=True,specialEnergy=0)),a('specialDrain',value=100)]
   add(actions,failure=dict(event=event,nth=nth),weights={'1':dict(value='2.5')})
 for event in events:
  for mutation in [a('state',fields=dict(musicVolume=17,publicChatSetting=253,privateChatSetting=252,tradeSetting=251,weight='9.75',specialToggled=True,specialEnergy=-1)),a('configure',fields=dict(retaliating=True,inventoryId=2,dispatcherNull=True)),a('specialSet',value=True)]:
   add([a('slots',id=0,slots=[1]+[None]*27),a('slots',id=1,slots=[1]+[None]*13),a('slots',id=2,slots=[None]*28),a('update'),a('weight'),a('acceptAid'),a('retaliate'),a('specialDrain',value=100)],weights={'1':dict(value='2.5')},hooks=[dict(event=event,nth=1,actions=[mutation])])
 # Inventory reads the field after getConfiguration; equipment compound += captures before it.
 for container in [0,1]:
  add([a('slots',id=container,slots=[1]+[None]*(27 if container==0 else 13)),a('weight')],weights={'1':dict(value='2.0')},hooks=[dict(event='weight',nth=1,actions=[a('state',fields=dict(weight='10.0'))])])
 for config in [dict(sessionNull=True),dict(dispatcherNull=True),dict(propertiesNull=True),dict(ironmanNull=True),dict(inventoryId=-1),dict(equipmentId=-1)]:add([a('update'),a('chat'),a('weight'),a('retaliate'),a('acceptAid')],**config)
 for value in [None,'true',True,False,1]:
  row=dict(defaults);row['retaliation']=value
  add([a('parse',data=row)],propertiesNull=True)
 add([a('weight')],hooks=[dict(event='dispatcher',nth=1,actions=[a('state',fields=dict(weight='9.75'))])])
 add([a('chat')],hooks=[dict(event='session',nth=1,actions=[a('setChat',public=255,private=254,trade=253)])])
 add([a('retaliate')],hooks=[dict(event='properties',nth=1,actions=[a('configure',fields=dict(propertiesNull=True))]),dict(event='properties',nth=2,actions=[a('configure',fields=dict(propertiesNull=False))])])
 rng=random.Random(530)
 for _ in range(80):
  actions=[]
  for _ in range(12):
   op=rng.choice(setters+['mouse','effects','split','aid','runToggle','specialToggle','specialDrain','retaliate','update','chat'])
   actions.append(a(op,value=rng.choice([-2147483648,-1,0,1,255,2147483647])))
  add(actions)
 return cases

ADAPTERS={
'core/game/system/config/ItemConfigParser.java':'package core.game.system.config;public class ItemConfigParser{public static String WEIGHT="weight";}',
'core/api/ContentAPIKt.java':'package core.api;public class ContentAPIKt{public static void setVarp(core.game.node.entity.player.Player p,int id,int value){oracle.Support.emit("varp",id,value);}}',
'core/net/packet/IoBuffer.java':'package core.net.packet;public class IoBuffer{public final int opcode;public final org.json.simple.JSONArray bytes=new org.json.simple.JSONArray();public IoBuffer(int id){opcode=id;}public IoBuffer put(int value){bytes.add(value&255);return this;}}',
'core/game/node/item/Item.java':'package core.game.node.item;public class Item{final int id;public Item(int id){this.id=id;}public oracle.Support.Definition getDefinition(){oracle.Support.emit("definition",id);return new oracle.Support.Definition(id);}}',
'core/game/node/entity/player/Player.java':'''package core.game.node.entity.player;public class Player{
public oracle.Support.Properties getProperties(){oracle.Support.emit("properties");return oracle.Support.flag("propertiesNull")?null:oracle.Support.properties;}
public oracle.Support.Ironman getIronmanManager(){oracle.Support.emit("ironmanManager");return oracle.Support.flag("ironmanNull")?null:oracle.Support.ironman;}
public oracle.Support.Dispatcher getPacketDispatch(){oracle.Support.emit("dispatcher");return oracle.Support.flag("dispatcherNull")?null:oracle.Support.dispatcher;}
public oracle.Support.Session getSession(){oracle.Support.emit("session");return oracle.Support.flag("sessionNull")?null:oracle.Support.session;}
public oracle.Support.Container getInventory(){return oracle.Support.container("inventory");}
public oracle.Support.Container getEquipment(){return oracle.Support.container("equipment");}
}'''}
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
 run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
 jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 source=ref/'Server/src/main/core/game/node/entity/player/link/Settings.java';read(source);read(HERE/'Support.java');files=[source,HERE/'Support.java']
 for name,text in ADAPTERS.items():
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');files.append(p)
 cp=os.pathsep.join(map(str,[classes,jar]));run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*files])
 inputs=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in inputs),encoding='utf-8')
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:r=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if r.returncode:raise RuntimeError(r.stderr.decode(errors='replace'))
 rows=[json.loads(l)for l in r.stdout.splitlines()if l];assert len(rows)==len(inputs)
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),scope='Complete unchanged Settings.java. Player getters/properties, captured container/item/definition identities, config lookup, packet dispatch, IO buffer sink and synchronous callback/failure hooks are explicit adapters. Not complete Player, item/content integration, profile lifecycle or live settings proof.')
 folder=ROOT/'fixtures/settings';folder.mkdir(exist_ok=True)
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Settings fixtures/provenance differ')
  print(f'{len(rows)} original Settings scenarios reproduced.')
 else:
  (folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n');print(f'Generated {len(rows)} original Settings scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);args=p.parse_args();c.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(args.check)
