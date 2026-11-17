"""Whole original BankInterface, InterfaceListener/Listeners and settings builder."""
import argparse,copy,hashlib,importlib.util,json,os,pathlib,subprocess,shutil
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).parent;O=R/'runs/bank-interface-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',R/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
def requests():
 rows=[]
 def a(op,**kw):return dict(op=op,**kw)
 def add(actions,**kw):rows.append(copy.deepcopy(dict(actions=actions,**kw)))
 for iface,childs in [(0,[10,14,16,18,20,22,23,73,*range(25,42)]),(1,[0,1,73]),(2,[9,10,22]),(3,[0])]:
  for child in childs:
   add([a('run',component=iface,button=child,opcode=155),a('run',kind='open',component=iface),a('run',kind='close',component=iface)])
 for opcode in [-2147483648,-1,0,9,124,132,155,166,168,196,199,234,244,2147483647]:
  for component in [0,1]:
   for slot in [-1,0,1,496]:add([a('run',component=component,button=73,opcode=opcode,slot=slot,item=999)])
 for amount in [-2147483648,-1,0,99999,100000,100001,2147483647]:
  for ident in [995,1351]:
   add([a('set',item=0,amount=amount),a('set',definition=0,id=ident),a('run',component=0,button=73,opcode=9)])
 for name in [None,'','Coins','Rune \U0001f43e']:
  for examine in [None,'','Lovely money!']:
   add([a('set',definition=0,name=name,examine=examine),a('run',component=0,button=73,opcode=9)])
 for value in [None,False,True,'bad',0,-1,9,10,2147483648]:
  add([a('attr',key='bank:lasttab',value=value),a('run',kind='open',component=0)])
 for kind in ['open','close','button']:
  for player,component in [(None,0),(0,None),(None,None),(1,0)]:add([a('run',kind=kind,player=player,component=component,button=73,opcode=155)])
 add([a('get',name='oldOpen',kind='open',id=762),a('get',name='oldMain',id=762),a('get',name='oldHelp',id=767),a('define',program=1),a('get',name='newOpen',kind='open',id=762),a('get',name='newMain',id=762),a('get',name='newHelp',id=767),a('same',left='oldOpen',right='newOpen'),a('same',left='oldMain',right='newMain'),a('same',left='oldHelp',right='newHelp')])
 for kind in ['open','close','button']:
  for specific in [False,True]:
   add([a('callback',name='a',kind='open'if kind!='button'else'button',label=7),a('callback',name='b',kind='open'if kind!='button'else'button',label=7),a('alias',name='alias',source='a'),a('same',left='a',right='b'),a('same',left='a',right='alias'),a('map',map='open'if kind!='button'else'buttons',key=('close:762'if kind=='close'else'762:73'if specific and kind=='button'else'762'),value='a'),a('run',kind=kind,component=0,button=73,opcode=155),a('map',map='open'if kind!='button'else'buttons',key=('close:762'if kind=='close'else'762:73'if specific and kind=='button'else'762'),value=None),a('run',kind=kind,component=0,button=73,opcode=155)])
 for phase in ['bank','inventory','item','lastTab','setTab','varc','definition','examine','id','amountId','amount','name','last','take','deposit','dispatch','settings','insert','setInsert','note','setNote','dialogueCreate','dialogue','message','transfer','debug','openBank']:
  for at in [1,2]:
   add([a('run',kind='open',component=0),*[a('run',component=0,button=button,opcode=opcode)for button,opcode in [(14,155),(16,155),(73,199),(73,168),(73,9),(73,234),(23,155),(18,155),(73,0)]],a('run',component=1,opcode=168),a('run',component=2,button=10,opcode=155)],failures={phase:at})
 for phase in ['bank','inventory','item','lastTab','setTab','varc','definition','examine','id','amountId','amount','last','insert','note','dispatch','settings']:
  for change in [a('set',player=0,bank=2,inventory=3),a('set',player=0,bank=None),a('set',container=0,last=999,insert=True,note=True),a('set',item=0,amount=7,definition=1),a('set',definition=0,id=1351,name='new',examine='changed')]:
   add([a('run',kind='open',component=0),a('run',component=0,button=14,opcode=155),a('run',component=0,button=16,opcode=155),a('run',component=0,button=73,opcode=199),a('run',component=0,button=73,opcode=168),a('run',component=0,button=73,opcode=9),a('run',component=1,opcode=168)],hooks=[dict(event=phase,actions=[change])])
 for change in [a('map',map='buttons',key='762:73',value='b'),a('result',name='a',value=False),a('set',component=0,id=999),a('run',component=0,button=73,opcode=155)]:
  add([a('callback',name='a',kind='button',label=7),a('callback',name='b',kind='button',label=7),a('map',map='buttons',key='762:73',value='a'),a('run',component=0,button=73,opcode=155),a('run',component=0,button=73,opcode=155)],hooks=[dict(event='callback',actions=[change])])
 spec=importlib.util.spec_from_file_location('packets',H/'packet_cases.py');packets=importlib.util.module_from_spec(spec);spec.loader.exec_module(packets);c.read(H/'packet_cases.py');rows.extend(packets.requests())
 for size in [-1,0,496,65536,2147483647]:add([a('set',bankSize=size),a('run',kind='open',component=0)])
 add([a('run',kind='open',component=0)],hooks=[dict(event='dispatch',actions=[a('set',bankSize=1234)])])
 for kind in ['open','close','button']:
  key='close:762'if kind=='close'else'762';mapping='buttons'if kind=='button'else'open'
  add([a('callback',name='wrong',kind='open'if kind=='button'else'button'),a('map',map=mapping,key=key,value='wrong'),a('get',kind=kind,id=762,name='got'),a('run',kind=kind,component=0,button=73,opcode=155),a('map',map=mapping,key=key,value=None),a('get',kind=kind,id=762,name='cleared')])
 add([a('callback',name='wrong',kind='open'),a('map',map='buttons',key='762:73',value='wrong'),a('get',id=762,button=73,name='got'),a('run',component=0,button=73,opcode=155)])
 for target in ['bank','inventory']:
  add([a('set',item=0,definition=None),a('run',component=0 if target=='bank'else 1,button=73,opcode=9)])
 add([a('set',definition=0,id=1351,examine=None),a('run',component=0,button=73,opcode=9)])
 return rows
def generate(check):
 lock=json.loads(c.read(R/'upstream.lock.json'));assert c.run(['git','-C',R/'reference/server','rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];c.run(['git','-C',R/'reference/server','diff','--quiet','HEAD','--'])
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[];kt=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(java if name.endswith('.java')else kt).append(p)
 def source(name):
  p=S/name;c.read(p);(java if name.endswith('.java')else kt).append(p)
 for name in ['content/global/handlers/iface/bank/BankInterface.kt','core/game/interaction/InterfaceListener.kt','core/game/interaction/InterfaceListeners.kt','core/api/IfaceSettingsBuilder.kt']:source(name)
 processor=c.read(S/'core/net/packet/PacketProcessor.kt');start=processor.index('    private fun processCloseIface(');end=processor.index('    private fun processUseWith(',start);methods=processor[start:end]
 adapter('oracle/Processor.kt',c.read(H/'Processor.kt').replace('/* ORIGINAL_METHODS */',methods))
 adapter('core/net/packet/in/Packet.kt',c.read(H/'Packet.kt'))
 adapter('core/game/world/GameWorld.kt',c.read(H/'GameWorld.kt'))
 adapter('core/game/system/task/Pulse.kt','package core.game.system.task\nabstract class Pulse { abstract fun pulse():Boolean }')
 adapter('core/game/event/ButtonClickEvent.java','package core.game.event;public class ButtonClickEvent {public final int id,child;public ButtonClickEvent(int i,int c){id=i;child=c;}}')
 adapter('core/game/node/entity/player/info/login/LoginConfiguration.java','package core.game.node.entity.player.info.login;public class LoginConfiguration{public static void configureGameWorld(core.game.node.entity.player.Player p){oracle.Oracle.emit("configureWorld",p.id);}}')
 for name in ['Player.java','Container.java','Oracle.java']:
  p=H/name;c.read(p);java.append(p)
 adapter('core/api/ContentAPI.kt',c.read(H/'ContentAPI.kt'))
 adapter('core/ServerConstants.java','package core;public class ServerConstants{public static int BANK_SIZE=496;}')
 adapter('core/game/component/Component.java',c.read(H/'Component.java'))
 adapter('core/game/component/ComponentPlugin.java',c.read(H/'ComponentPlugin.java'))
 adapter('content/global/handlers/iface/bank/BankUtils.kt','package content.global.handlers.iface.bank\nobject BankUtils {fun transferX(p:core.game.node.entity.player.Player,slot:Int,withdraw:Boolean,after:(()->Unit)?=null){oracle.Oracle.emit("transfer",p.id,slot,withdraw)}}')
 for name,kind in [('BankHelpDialogue',0),('BankDepositDialogue',1)]:adapter('content/global/dialogue/'+name+'.java',f'package content.global.dialogue;public class {name}{{public {name}(){{oracle.Oracle.emit("dialogueCreate",{kind});}}}}')
 const=json.loads(c.read(R/'oracle/bank/constants.lock.json'));jar=R/'runs/bank-oracle/ConstLib-1.4.28.jar';assert jar.stat().st_size==const['size']and hashlib.sha256(jar.read_bytes()).hexdigest()==const['sha256']
 jlock=json.loads(c.read(R/'oracle/auth/json.lock.json'));jsonjar=R/'runs/interfaces-oracle/json-simple-1.1.1.jar';assert hashlib.sha256(jsonjar.read_bytes()).hexdigest()==jlock['sha256']
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));cp=os.pathsep.join(map(str,[classes,std,jar,jsonjar]));c.run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 rows=requests();p=O/'input.jsonl';p.write_text(''.join(json.dumps(row)+'\n'for row in rows),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Oracle'])),cwd=R,stdin=f,capture_output=True,timeout=90)
 assert result.returncode==0,result.stderr.decode(errors='replace');data=[json.loads(line)for line in result.stdout.splitlines()];assert len(data)==len(rows);fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in data).encode();c.read(pathlib.Path(__file__))
 initial=json.loads(c.read(H/'initial.lock.json'));assert hashlib.sha256(b''.join(fixture.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256'];packets_initial=json.loads(c.read(H/'packet.lock.json'));assert hashlib.sha256(b''.join(fixture.splitlines(keepends=True)[:packets_initial['caseCount']])).hexdigest()==packets_initial['sha256'];provenance=dict(format=1,initialBankInterfaceBaseline=initial,initialInterfacePacketBaseline=packets_initial,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),externalConstants=const,scope='Complete unchanged BankInterface, InterfaceListener/InterfaceListeners and IfaceSettingsBuilder; exact selected PacketProcessor IfAction/CloseIface bodies and captured anonymous login pulse. Player/container/item/definition/dispatch/attribute/dialogue/BankUtils APIs are explicit mutable recording services; bank item mutation and deferred amount input are not replaced by these adapters. Actual Kotlin callback identities, equal-label distinct objects, aliases, raw maps, late receivers and failure/reentry prefixes. Component/plugin/dialogue/lock/zone/event/scheduler/login methods are explicit mutable recording services. Not full Player, packet processor class, content/startup or live banking proof.')
 folder=R/'fixtures/bank-interface';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
 print(len(rows),'original bank interface/registry scenarios reproduced'if check else'original bank interface/registry scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
