"""Complete unchanged BankContainer/BankListener, original builder and packet writer."""
import argparse,copy,hashlib,importlib.util,itertools,json,os,pathlib,re,shutil,subprocess,urllib.request
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).parent;O=R/'runs/bank-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('inventory',R/'oracle/inventory-listener/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);c=base.c
def requests():
 rows=[]
 defs={str(i):dict(id=i,name='item'+str(i),stackable=i in [0,2,4],unnoted=i!=4,noteId=2 if i==1 else 3 if i==4 else -1)for i in range(6)}
 def add(actions,**kw):
  row=dict(definitions=defs,items=dict(a=dict(id=1,amount=3,charge=1000),b=dict(id=1,amount=2,charge=500),note=dict(id=4,amount=12,charge=77),plain=dict(id=3,amount=12,charge=77),stack=dict(id=2,amount=2147483640),nil=None),inventories=[{'0':'a','1':'b','2':'note'},{}],banks=[{'0':'a','1':'plain'},{'0':'b'},{}],tabs=[[0]*11 for _ in range(3)],actions=actions,hooks=[]);row.update(kw);rows.append(copy.deepcopy(row))
 add([dict(op='modes'),dict(op='space'),dict(op='maximum',item='note'),dict(op='canAdd'),dict(op='configs')])
 for value in [-2147483648,-1,0,1,2,9,10,11,21,23,37,39,41,74,2147483647]:
  add([dict(op='arrayIndex',value=value),dict(op='tab',value=value),dict(op='tabBySlot',slot=value),dict(op='itemsInTab',value=value),dict(op='last',value=value)])
 for slot,amount in itertools.product([-1,0,1,2,27,28,29],[-1,0,1,2,20,2147483647]):
  add([dict(op='deposit',slot=slot,amount=amount),dict(op='update'),dict(op='refresh')])
 for slot,amount,note in itertools.product([-1,0,1,495,496,497],[0,1,2,20,2147483647],[False,True]):
  add([dict(op='withdraw',slot=slot,amount=amount),dict(op='update'),dict(op='refresh')],noteMode=note)
 for blocked in [None,False,True,1,'bad']:
  d=copy.deepcopy(defs);d['1']['handlers']={'bankable':blocked}
  add([dict(op='canAdd'),dict(op='deposit',amount=3)],definitions=d)
 for item in ['a','note','plain','stack','nil']:
  add([dict(op='maximum',item=item),dict(op='baseAdd',item=item,fire=False),dict(op='update')])
 for note,insert in itertools.product([False,True],repeat=2):
  add([dict(op='note',value=note),dict(op='insert',value=insert),dict(op='modes')])
 for restricted,locked,dev,player in itertools.product([False,True],[False,True],[False,True],[-2,-1,0,1]):
  op=dict(op='open')if player==-2 else dict(op='openFor',player=player)
  add([op,op,dict(op='close'),dict(op='close')],restricted=restricted,locked=locked,dev=dev)
 for op in ['open','openFor','depositBox','refreshDeposit','close','deposit','withdraw','space','configs','note','insert','last','tab']:
  add([dict(op=op,player=1,amount=2,value=1)],nullPlayer=True)
 for tab in [0,1,2,8,9,10,-1,11]:
  tabs=[0,0,1,2,2,2,2,2,2,2,3]
  for op in ['increase','decrease','collapse']:
   add([dict(op='array'),dict(op=op,value=tab),dict(op='space')],tabs=[tabs,[0]*11,[0]*11],banks=[{'0':'a','1':'note','2':'b'}, {}, {}])
  add([dict(op='tab',value=tab),dict(op='deposit',slot=2,amount=2),dict(op='withdraw',slot=1,amount=1)],tabs=[tabs,[0]*11,[0]*11],banks=[{'0':'a','1':'note','2':'b'}, {}, {}])
 for bank in [0,1,2]:
  add([dict(op='depositBox',bank=bank),dict(op='closeCallback',value=0,player=1),dict(op='openFor',bank=bank,player=1),dict(op='closeCallback',value=1,player=0)])
 full={str(i):'a'for i in range(496)}
 add([dict(op='deposit',slot=2,amount=12)],banks=[full,{},{}])
 add([dict(op='withdraw',slot=0,amount=3)],inventories=[{str(i):'b'for i in range(28)},{}])
 for op,when in itertools.product(['open','openFor','depositBox','deposit','withdraw','close','collapse'],['inventory','bank','packet','ironman','restriction','pin','unlocked','dev','openPin','interfaces','component','open','openComponent','closeEvent','removeTabs','singleTab','closeSingle','dispatch','ifaceSettings','script','message','packetMessage','getVarbit','varbit','varp','varc','attribute','removeAttribute','depositItems','inventoryUpdate','inventoryRefresh']):
  add([dict(op=op,amount=2,value=1),dict(op='space')],hooks=[dict(when=when,action='fail')])
 for when,action in itertools.product(['inventory','bank','packet','varbit','interfaces','inventoryRefresh','inventoryUpdate'],['swapBank','clearInventory','clearBank','replaceInventory','amount','event','reenter','close']):
  add([dict(op='open'),dict(op='deposit',slot=2,amount=2),dict(op='withdraw',slot=0,amount=1)],hooks=[dict(when=when,action=action)])
 add([dict(op='open'),dict(op='array'),dict(op='clearListeners'),dict(op='clear'),dict(op='baseAdd',item='note'),dict(op='close')])
 for store,active,amount in itertools.product(['none','inventory','primary','secondary','equipment','otherbank'],[0,1],[0,1,3,2147483647]):
  inv=[{},{}];banks=[{},{},{}];equipment=[{},{}]
  target={'inventory':inv[0],'primary':banks[0],'secondary':banks[1],'equipment':equipment[0],'otherbank':banks[2]}.get(store)
  if target is not None:target['0']='a'
  items=dict(a=dict(id=1,amount=amount,charge=1000),b=dict(id=1,amount=2,charge=500),nil=None)
  add([dict(op='hasItem',item=item)for item in ['a','b','nil']],ownership=True,equipment=equipment,inventories=inv,banks=banks,activeBank=active,items=items)
 for when,action in itertools.product(['inventory','bank'],['swapBank','clearInventory','clearBank','replaceInventory','amount']):
  add([dict(op='hasItem',item='a'),dict(op='hasItem',item='b')],ownership=True,equipment=[{},{}],inventories=[{},{}],hooks=[dict(when=when,action=action)])
 return rows
def generate(check):
 baseline=R/'fixtures/inventory/oracle.jsonl';previous=hashlib.sha256(baseline.read_bytes()).hexdigest();base.generate(True);assert hashlib.sha256(baseline.read_bytes()).hexdigest()==previous
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[]
 lock=json.loads(c.read(H/'constants.lock.json'));jar=O/'ConstLib-1.4.28.jar';cache=R/'runs/bank-constants-investigation/ConstLib-1.4.28.jar'
 if cache.is_file()and hashlib.sha256(cache.read_bytes()).hexdigest()==lock['sha256']:shutil.copyfile(cache,jar)
 elif not jar.is_file()or hashlib.sha256(jar.read_bytes()).hexdigest()!=lock['sha256']:
  with urllib.request.urlopen(lock['url'],timeout=60)as response:data=response.read(lock['size']+1)
  assert len(data)==lock['size']and hashlib.sha256(data).hexdigest()==lock['sha256'];jar.write_bytes(data)
 assert jar.stat().st_size==lock['size']and hashlib.sha256(jar.read_bytes()).hexdigest()==lock['sha256']
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 constants=c.read(S/'core/ServerConstants.kt');size=int(re.search(r'BANK_SIZE: Int = (\d+)',constants).group(1));assert size==496
 adapter('core/ServerConstants.java',f'package core;public class ServerConstants{{public static int BANK_SIZE={size};}}')
 player=c.read(S/'core/game/node/entity/player/Player.java')
 adapter('core/game/node/entity/player/Player.java',c.read(H/'Player.java').replace('/* ORIGINAL_HAS_ITEM */',c.method(player,'public boolean hasItem(Item item)')))
 adapter('core/game/component/CloseEvent.java','package core.game.component;public interface CloseEvent{boolean close(core.game.node.entity.player.Player p,Component c);}')
 adapter('core/game/component/Component.java','package core.game.component;public class Component{public final int id;public Component(int n){id=n;oracle.BankOracle.hit("component",n);}public Component setCloseEvent(CloseEvent e){oracle.BankOracle.closeEvent(this,e);return this;}}')
 adapter('core/game/container/access/InterfaceContainer.java','package core.game.container.access;public class InterfaceContainer{public static void generateItems(core.game.node.entity.player.Player p,core.game.node.item.Item[]items,String[]options,int a,int b,int c,int d){oracle.BankOracle.hit("depositItems",p.id,options,a,b,c,d);}}')
 adapter('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{public static Settings getSettings(){return new Settings();}public static class Settings{public boolean isDevMode(){oracle.BankOracle.hit("dev");return oracle.BankOracle.flag("dev");}}}')
 config=c.read(S/'core/game/system/config/ItemConfigParser.kt')
 adapter('core/game/system/config/ItemConfigParser.java','package core.game.system.config;public class ItemConfigParser{'+''.join('public static String '+name+'='+re.search(r'const val '+name+r' = ("[^"]+")',config).group(1)+';'for name in ['HIGH_ALCHEMY','LOW_ALCHEMY','BANKABLE'])+'}')
 adapter('core/net/packet/PacketRepository.java','package core.net.packet;public class PacketRepository{public static void send(Class<?>t,core.net.packet.context.ContainerContext c){oracle.BankOracle.packet(c);}}')
 adapter('core/api/ContentAPIKt.java',r'''package core.api;import oracle.BankOracle;import core.game.node.entity.player.Player;public class ContentAPIKt{
 public static void log(Object o,int level,String text){throw new AssertionError(text);}
 public static int getVarbit(Player p,int id){BankOracle.hit("getVarbit",p.id,id);return BankOracle.vars.get(p.id).getOrDefault(""+id,0);}
 public static void setVarbit(Player p,int id,int value){BankOracle.hit("varbit",p.id,id,value,false);BankOracle.vars.get(p.id).put(""+id,value);}
 public static void setVarbit(Player p,int id,int value,boolean send){BankOracle.hit("varbit",p.id,id,value,send);BankOracle.vars.get(p.id).put(""+id,value);}
 public static void setVarp(Player p,int id,int value){BankOracle.hit("varp",p.id,id,value);}
 public static void setVarc(Player p,int id,int value){BankOracle.hit("varc",p.id,id,value);}
 public static void setAttribute(Player p,String key,Object value){BankOracle.hit("attribute",p.id,key,value);BankOracle.attributes.get(p.id).put(key,value);}
 }''')
 for name in ['core/game/container/impl/BankContainer.java','core/game/node/entity/player/link/IronmanMode.java']:
  p=S/name;c.read(p);java.append(p)
 java.append(H/'BankOracle.java');c.read(H/'BankOracle.java')
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
 cp=os.pathsep.join(map(str,[classes,std,jar,R/'runs/inventory-listener-oracle/classes',R/'runs/inventory-oracle/classes',R/'runs/inventory-oracle/json-simple-1.1.1.jar']))
 builder=S/'core/api/IfaceSettingsBuilder.kt';c.read(builder)
 c.run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,builder])
 c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();data=''.join(json.dumps(row)+'\n'for row in cases).encode();(O/'input.jsonl').write_bytes(data)
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 p=subprocess.run([str(exe),'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.BankOracle'],cwd=R,input=data,capture_output=True,timeout=120)
 (O/'stderr.log').write_bytes(p.stderr);assert p.returncode==0,p.stderr.decode(errors='replace')
 (O/'output.jsonl').write_bytes(p.stdout)
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases)
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();c.read(pathlib.Path(__file__))
 initial=json.loads(c.read(H/'initial.lock.json'));assert hashlib.sha256(b''.join(fixture.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256']
 provenance=dict(format=1,upstream=json.loads(c.read(R/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),unchangedInventoryBaseline=previous,initialBankBaseline=initial,externalConstants=lock,scope='Complete unchanged BankContainer including private BankListener, unchanged Player.hasItem method, complete original Kotlin IfaceSettingsBuilder, unchanged Item/Container/Event and actual ContainerContext/ContainerPacket/IoBuffer. Player getters, interfaces/close event storage, PIN/Ironman/world settings, var/attribute APIs and deposit rendering are explicit recording adapters. Covers active-bank/retained receiver differences, tabs/notes/charges, exposed arrays/events/listeners, open overloads and failure/reentry prefixes; not full Player/profile/bank UI/content startup or playable banking.')
 folder=R/'fixtures/bank';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
 print(len(rows),'complete original BankContainer scenarios reproduced'if check else'complete original BankContainer scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
