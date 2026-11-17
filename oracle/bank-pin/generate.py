"""Full original BankPinManager/PinStatus and BankPinInterface with explicit services."""
import argparse,copy,hashlib,importlib.util,itertools,json,os,pathlib,shutil,subprocess,urllib.request
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).parent;O=R/'runs/bank-pin-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',R/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
def requests():
 rows=[]
 def add(actions,**kw):
  row=dict(initial={},actions=actions,hooks=[],times=[],randoms=[],clock=1000000,seed=1);row.update(kw);rows.append(copy.deepcopy(row))
 def enter(s):return[dict(op='digit',value=int(d))for d in s]
 add([dict(op='getters'),dict(op='statusMessages'),dict(op='statusLogin'),dict(op='statusDraw'),dict(op='register'),dict(op='register')])
 for status,pin,unlocked,long in itertools.product([None,0,1,2],[None,'4938'],[False,True],[False,True]):
  add([dict(op='getters'),dict(op='statusMessages'),dict(op='statusLogin'),dict(op='statusDraw'),dict(op='login')],initial=dict(status=status,pin=pin,unlocked=unlocked,longRecovery=long,pendingDelay=260200000))
 for status,button,unlocked in itertools.product([0,1,2],[-1,0,1,2,3,4,5,2147483647],[False,True]):
  add([dict(op='type',value=button),dict(op='getters')],initial=dict(status=status,pin='4938'if status else None,unlocked=unlocked,pendingDelay=260200000))
 for pin,dev,long in itertools.product(['4938','8042','1234','0000','1212','9876','0198'],[False,True],[False,True]):
  add([dict(op='pin'),*enter(pin+pin),dict(op='getters'),dict(op='login')],dev=dev,initial=dict(longRecovery=long))
 for pin in ['49388042','80424938','12341234','11112222']:
  add([dict(op='pin'),*enter(pin),dict(op='settings')])
 for tries,correct,invoke in itertools.product([-2147483648,0,1,2,3,4,2147483647],[False,True],[False,True]):
  add([dict(op='type',value=1),*enter('4938'if correct else'8042'),dict(op='getters'),dict(op='pin')],initial=dict(status=2,pin='4938',tries=tries),invokeClose=invoke)
 for change,pin in itertools.product([0,1,2],['4938','8042','1234']):
  add([dict(op='pin'),*enter('4938'),*enter(pin+pin),dict(op='getters')],initial=dict(status=2,pin='4938',changeState=change))
 for status,confirm,unlocked in itertools.product([None,0,1,2],[89,91,0],[False,True]):
  add([dict(op='confirm',value=confirm),dict(op='getters')],initial=dict(status=status,pin='4938'if status else None,unlocked=unlocked))
 for temp,stage in itertools.product([None,'','1','1234','12341234','49384938','12121212','éééééééé','😀😀😀😀'],[-1,0,3,4,5,8,9,2147483647]):
  add([dict(op='stage',value=stage),dict(op='getters')],initial=dict(tempPin=temp,pin='4938',status=2))
 for button in [-2147483648,-1,0,1,9,10,2147483647]:
  add([dict(op='click',value=button)])
 for delay,now,dev in itertools.product([-9223372036854775808,-1,0,999999,1000000,1000001,260200000,9223372036854775807],[-9223372036854775808,1000000,9223372036854775807],[False,True]):
  add([dict(op='days'),dict(op='blocked'),dict(op='login'),dict(op='settings')],initial=dict(status=1,pin='4938',pendingDelay=delay,tryDelay=delay),clock=now,dev=dev)
 for times in [[1000000,1000001,1000002],[1000000,260200001,260200002],[260200000,260200000],[260199999,260200000,260200001]]:
  add([dict(op='type',value=1),dict(op='login')],times=times,initial=dict(status=1,pin='4938',pendingDelay=260200000))
 for data in [None,{},[],dict(pin=None),dict(pin=1234,status=2),dict(pin='4938',status=-1),dict(pin='4938',status=3),dict(pin='4938',status=None),dict(status='bad'),dict(pin='4938',status=2,pendingDelay='9223372036854775808'),dict(pin='4938',status=2,pendingDelay='-9223372036854775808',tryDelay='9223372036854775807',longRecovery=True),dict(pin='4938',status=2,pendingDelay=5,tryDelay=6,longRecovery=None),dict(pin='4938',status=2,pendingDelay=5,tryDelay=6,longRecovery=1),dict(pin='4938',status=2,pendingDelay=5,tryDelay=6,longRecovery=False)]:
  add([dict(op='parse',data=data),dict(op='getters')])
 for messages in [None,[],['x',None,'','y'],['a']*16]:
  add([dict(op='messages',messages=messages),dict(op='cancel',messages=messages)],initial=dict(tempPin='4938',pin='4938',status=2,unlocked=True,deleting=True,changeState=2))
 for null,op in itertools.product(['interfaces','dispatch','bank','dialogue','settings','name','exchange'],['settings','pin','login','type','toggle','stage']):
  add([dict(op=op,value=1)],null=null,initial=dict(pin='4938',status=2,tempPin='8042',tries=2,tryDelay=1000005))
 for op in ['settings','pin','login','type','toggle','stage','switch','getters','doCancel']:
  add([dict(op=op,value=1)],nullOwner=True)
 operations=[dict(op='settings'),dict(op='pin'),dict(op='type',value=1),dict(op='toggle',value=True),dict(op='stage',value=4),dict(op='stage',value=8),dict(op='switch'),dict(op='login'),dict(op='register'),dict(op='handle',component=13,value=1)]
 labels=['clock','interfaces','close','dispatch','string','config','packetString','audio','component','closeEvent','open','bank','openBank','exchange','collection','market','settings','name','dev','varp','random','position','dialogue','sendDialogue','message','packetMessage','pinManager','componentId','definition','plugin']
 for action,label in itertools.product(operations,labels):
  add([action,dict(op='getters')],initial=dict(pin='4938',status=2,tempPin='80428042',tries=1),hooks=[dict(when=label,action='fail')])
 for label,field,value in itertools.product(['clock','interfaces','close','dispatch','string','config','packetString','audio','open','varp','random','position'],['status','tempPin','changeState','longRecovery'],[0,1]):
  values={'status':[0,None],'tempPin':[None,'49'],'changeState':[0,2],'longRecovery':[False,True]}
  add([dict(op='pin'),dict(op='stage',value=8)],initial=dict(pin='4938',status=2,tempPin='49384938'),hooks=[dict(when=label,action='set',field=field,value=values[field][value])])
 for component,button in itertools.product([13,14,99],[-2147483648,-1,0,1,10,30,60,61,62,63,64,65,89,91,2147483647]):
  add([dict(op='handle',component=component,value=button)],initial=dict(status=2,pin='4938'))
 add([dict(op='pin'),dict(op='replaceManager'),dict(op='set',target=0,field='changeState',value=2),dict(op='set',target=0,field='deleting',value=True),dict(op='closeEvent',value=0),dict(op='getters',target=2)])
 for nullcomponent,player in itertools.product([False,True],[-1,0,1]):add([dict(op='handle',component=13,value=30,nullComponent=nullcomponent,player=player)])
 for label in ['pinManager','componentId']:
  add([dict(op='handle',component=13,value=1)],hooks=[dict(when=label,action='replaceManager')])
  add([dict(op='handle',component=13,value=30)],hooks=[dict(when=label,action='nullManager')])
 for label in ['definition','plugin']:
  for at in [1,2]:add([dict(op='register')],hooks=[dict(when=label,at=at,action='fail')])
 for seed in [-9223372036854775808,-1,0,1,530,9223372036854775807]:
  add([dict(op='pin'),*enter('4938')],seed=seed,randoms=[2,3,17,25,44,47])
 for correct,invoke in itertools.product([False,True],[False,True]):
  add([dict(op='confirm',value=89),*enter('4938'if correct else'8042'),dict(op='getters')],initial=dict(status=2,pin='4938'),invokeClose=invoke)
 for pin,current in itertools.product(['4938','8042',None],['49384938','80428042']):
  add([dict(op='stage',value=8)],initial=dict(status=2,pin=pin,tempPin=current,changeState=2))
 for field,value in [('tempPin','49'),('tempPin',None),('longRecovery',True),('status',None)]:
  add([dict(op='stage',value=8)],initial=dict(tempPin='49384938'),hooks=[dict(when='clock',action='set',field=field,value=value)])
 for label in ['close','dispatch','string','varp','position']:
  add([dict(op='pin'),dict(op='stage',value=8)],initial=dict(tempPin='49384938'),hooks=[dict(when=label,action='reenter',button=4)])
 return rows

def generate(check):
 lock=json.loads(c.read(R/'upstream.lock.json'));assert c.run(['git','-C',R/'reference/server','rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];c.run(['git','-C',R/'reference/server','diff','--quiet','HEAD','--'])
 O.mkdir(parents=True,exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[]
 def artifact(entry,target,candidates):
  def valid(p):return p.is_file()and p.stat().st_size==entry['size']and hashlib.sha256(p.read_bytes()).hexdigest()==entry['sha256']
  if not valid(target):
   cached=next((p for p in candidates if valid(p)),None)
   if cached:shutil.copyfile(cached,target)
   else:
    with urllib.request.urlopen(entry['url'],timeout=60)as response:data=response.read(entry['size']+1)
    assert len(data)==entry['size']and hashlib.sha256(data).hexdigest()==entry['sha256'];target.write_bytes(data)
  assert valid(target);return target
 jsonlock=json.loads(c.read(R/'oracle/auth/json.lock.json'));jsonjar=artifact(jsonlock,O/'json-simple-1.1.1.jar',[R/'runs/variables-oracle/json-simple-1.1.1.jar',R/'runs/inventory-oracle/json-simple-1.1.1.jar'])
 constants=json.loads(c.read(R/'oracle/bank/constants.lock.json'));jar=artifact(constants,O/'ConstLib-1.4.28.jar',[R/'runs/bank-oracle/ConstLib-1.4.28.jar'])
 cp=os.pathsep.join(map(str,[classes,jsonjar,jar]))
 def adapter(name,value):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(value,encoding='utf-8',newline='\n');java.append(p)
 source=c.read(S/'core/game/node/entity/player/link/BankPinManager.java');assert source.count('System.currentTimeMillis()')==7
 adapter('core/game/node/entity/player/link/BankPinManager.java',source.replace('System.currentTimeMillis()','oracle.PinOracle.now()'))
 p=S/'content/global/handlers/iface/bank/BankPinInterface.java';c.read(p);java.append(p)
 adapter('core/game/node/entity/player/Player.java',c.read(H/'Player.java'));adapter('oracle/PinOracle.java',c.read(H/'PinOracle.java'))
 adapter('core/game/component/CloseEvent.java',c.read(S/'core/game/component/CloseEvent.java'))
 adapter('core/game/component/Component.java','package core.game.component;public class Component{public final int id;public CloseEvent close;public Component(int id){this.id=id;oracle.PinOracle.hit("component",id);}public int getId(){oracle.PinOracle.hit("componentId",id);return id;}public Component setCloseEvent(CloseEvent event){oracle.PinOracle.event(this,event);return this;}}')
 adapter('core/game/component/ComponentDefinition.java','package core.game.component;public class ComponentDefinition{int id;ComponentDefinition(int id){this.id=id;}public static ComponentDefinition forId(int id){oracle.PinOracle.hit("definition",id);return new ComponentDefinition(id);}public void setPlugin(ComponentPlugin p){oracle.PinOracle.hit("plugin",id,p==oracle.PinOracle.program?0:1);oracle.PinOracle.plugins.put(id,p);}}')
 adapter('core/plugin/Plugin.java','package core.plugin;public interface Plugin<T>{Plugin<T> newInstance(T arg)throws Throwable;}')
 adapter('core/plugin/Initializable.java','package core.plugin;public @interface Initializable{}')
 adapter('core/game/component/ComponentPlugin.java','package core.game.component;public abstract class ComponentPlugin implements core.plugin.Plugin<Object>{public abstract boolean handle(core.game.node.entity.player.Player p,Component c,int opcode,int button,int slot,int itemId);}')
 adapter('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{public static Settings getSettings(){oracle.PinOracle.hit("settings");return oracle.PinOracle.nil("settings")?null:new Settings();}public static class Settings{public String getName(){oracle.PinOracle.hit("name");return oracle.PinOracle.nil("name")?null:"2009scape";}public boolean isDevMode(){oracle.PinOracle.hit("dev");return oracle.PinOracle.flag("dev");}}}')
 adapter('core/game/ge/GrandExchangeRecords.java','package core.game.ge;import core.game.node.entity.player.Player;public class GrandExchangeRecords{int id;GrandExchangeRecords(int id){this.id=id;}public static GrandExchangeRecords getInstance(Player p){oracle.PinOracle.hit("exchange",p.id);return oracle.PinOracle.nil("exchange")?null:new GrandExchangeRecords(p.id);}public void openCollectionBox(){oracle.PinOracle.hit("collection",id);}}')
 adapter('content/global/handlers/iface/ge/StockMarket.java','package content.global.handlers.iface.ge;public class StockMarket{public static void openFor(core.game.node.entity.player.Player p){oracle.PinOracle.hit("market",p.id);}}')
 adapter('core/tools/RandomFunction.java','package core.tools;public class RandomFunction{public static int random(int min,int max){return oracle.PinOracle.random(min,max);}}')
 adapter('core/api/ContentAPIKt.java','package core.api;import core.game.node.entity.player.Player;public class ContentAPIKt{public static void playAudio(Player p,int id){oracle.PinOracle.hit("audio",p.id,id);}public static void setVarp(Player p,int id,int value){oracle.PinOracle.hit("varp",p.id,id,value);}}')
 adapter('core/net/packet/context/StringContext.java','package core.net.packet.context;public class StringContext{public core.game.node.entity.player.Player p;public String text;public int component,child;public StringContext(core.game.node.entity.player.Player p,String t,int c,int h){this.p=p;text=t;component=c;child=h;}}')
 adapter('core/net/packet/context/ChildPositionContext.java','package core.net.packet.context;public class ChildPositionContext{public core.game.node.entity.player.Player p;public int component,child,x,y;public ChildPositionContext(core.game.node.entity.player.Player p,int c,int h,int x,int y){this.p=p;component=c;child=h;this.x=x;this.y=y;}}')
 adapter('core/net/packet/out/StringPacket.java','package core.net.packet.out;public class StringPacket{}');adapter('core/net/packet/out/RepositionChild.java','package core.net.packet.out;public class RepositionChild{}')
 adapter('core/net/packet/PacketRepository.java','package core.net.packet;import core.net.packet.context.*;public class PacketRepository{public static void send(Class<?>type,Object context){if(context instanceof StringContext){StringContext c=(StringContext)context;oracle.PinOracle.hit("packetString",c.p.id,oracle.PinOracle.units(c.text),c.component,c.child);}else{ChildPositionContext c=(ChildPositionContext)context;oracle.PinOracle.hit("position",c.p.id,c.component,c.child,c.x,c.y);}}}')
 c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();data=''.join(json.dumps(row)+'\n'for row in cases).encode();(O/'input.jsonl').write_bytes(data)
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 p=subprocess.run(list(map(str,[exe,'-Xmx512m','--add-opens=java.base/java.util=ALL-UNNAMED','-Dfile.encoding=UTF-8','-cp',cp,'oracle.PinOracle'])),cwd=R,input=data,capture_output=True,timeout=120);(O/'stderr.log').write_bytes(p.stderr);(O/'output.jsonl').write_bytes(p.stdout);assert p.returncode==0,p.stderr.decode(errors='replace')
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases)
 for a,b in zip(cases,out):assert a=={k:v for k,v in b.items()if k!='expected'}
 raw=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in out).encode();c.read(pathlib.Path(__file__));c.read(R/'oracle/login/generate.py')
 initial=json.loads(c.read(H/'initial.lock.json'));assert hashlib.sha256(b''.join(raw.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256']
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(out),fixtureSha256=hashlib.sha256(raw).hexdigest(),sourceHashAlgorithm='sha256, CR removed',constantsArtifact=constants,initialBaseline=initial,clockSubstitutions=7,scope='Complete original BankPinManager/PinStatus with only7 System.currentTimeMillis calls redirected to a controlled clock; unchanged full BankPinInterface including registration/handle. Original Collections.shuffle with private static Random explicitly seeded by test-only JDK11 reflection. Player/UI/packet/varp/audio/GE/world services and RandomFunction samples are recording adapters. Private initial-state injection is an explicit fixture adapter; Java UTF16 pin/message units are compared. Does not establish full interface/plugin/session/profile/content startup, random distribution of RandomFunction, emitted wire frames, full reflection/subclass API or live banking.')
 folder=R/'fixtures/bank-pin';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==raw and json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
 else:(folder/'oracle.jsonl').write_bytes(raw);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
 print(len(out),'original bank PIN scenarios reproduced'if check else'original bank PIN scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
