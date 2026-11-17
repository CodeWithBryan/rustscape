"""Pinned component metadata, original interface opening and gameframe methods."""
import argparse,hashlib,importlib.util,json,os,pathlib,re,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/interfaces-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests(config):
 cases=[]
 def a(op,**kw):return dict(op=op,**kw)
 def add(actions=None,**kw):cases.append(dict(actions=actions or[a('game')],**kw))
 for mode in [-1,0,1,2,3,4]:
  for familiar in [False,True]:add(mode=mode,familiar=familiar,configureUI=True,config=config)
 for typ in [None,*range(9)]:
  for operation in ['open','windows','chat','tab','tabAuto']:
   actions=[a('definition',id=149,type=typ,plugin=3),a('create',id=149),a(operation,component=0,slot=3)];add(actions)
 for kind in ['overlap','typed']:
  for value in [0,1,-1,2,255]:add([a('create',id=548),a('windows',component=0,**{kind:value})])
 for counter in [0,65535,2147483647,-2147483648]:add([a('state',counter=counter),a('game'),a('switch',mode=1),a('switch',mode=2)],configureUI=True,config=config)
 for slot in [-1,0,3,7,14,15]:
  for weapon in [False,True]:add([a('create',id=92,weapon=weapon),a('tab',component=0,slot=slot)])
 for event_close in [False,True]:
  for close_allowed in [False,True]:add([a('create',id=149),a('close',component=0),a('component',component=0,closeEvent=4),a('close',component=0)],eventClose=event_close,closeAllowed=close_allowed)
 for key in ['interfaceType','walkable','tabIndex']:
  for value in [None,'',True,0,-1,9,' 2','2147483648','false']:
   row=dict(id=149,interfaceType='2',walkable='true',tabIndex='3');row[key]=value
   add([a('definition',id=149,plugin=7),a('create',id=149),a('load',text=json.dumps([row])),a('create',id=149),a('plugin',component=0),a('plugin',component=1)])
 for malformed in ['null','{}','[null]','[{}]','[',json.dumps([dict(id=149,interfaceType=2,walkable=True,tabIndex=3),dict(id=149,interfaceType=9,tabIndex=8)])]:add([a('load',text=malformed)])
 for phase in ['send','log','listenerOpen','plugin','listenerClose','closeEvent','extension','weaponCreate','addWeapon','spellbook','familiar','friendsCreate','autocast','autoId','autoSelect','tutorial','dispatch','welcome','configure','appearance']:
  for at in [1,2]:add([a('game'),a('switch',mode=1)],configureUI=True,autocast=8,config=config,failures={phase:at})
 for phase in ['send','log','listenerOpen','plugin','extension','weaponCreate','addWeapon','spellbook','familiar','friendsCreate','autocast','tutorial','dispatch','configure']:
  for action in [a('state',counter=-1,window=None,opened=0),a('component',component=0,id=149,plugin=4),a('definition',id=746,type=2,walkable=True,tab=3),a('state',tabs=[None]*9),a('state',mode=1)]:add([a('game'),a('switch',mode=1)],configureUI=True,config=config,hooks=[dict(event=phase,actions=[action])])
 add([a('create',id=746),a('component',component=0,definition=None),a('open',component=0)])
 add([a('state',clientNull=True),a('game'),a('switch',mode=1)],config=config)
 add([a('create',id=137),a('chat',component=0),a('chat',component=0),a('create',id=999),a('chat',component=1),a('chat',component=0)])
 spec=importlib.util.spec_from_file_location('interface_object_cases',HERE/'object_cases.py');objects=importlib.util.module_from_spec(spec);spec.loader.exec_module(objects);read(HERE/'object_cases.py');cases.extend(objects.requests())
 spec=importlib.util.spec_from_file_location('interface_manager_cases',HERE/'manager_cases.py');managers=importlib.util.module_from_spec(spec);spec.loader.exec_module(managers);read(HERE/'manager_cases.py');cases.extend(managers.requests())
 return cases
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
 run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 folder=ROOT/'fixtures/interfaces';folder.mkdir(parents=True,exist_ok=True);raw=run(['git','-C',ref,'show',lock['server']['commit']+':Server/data/configs/interface_configs.json']).stdout;config=folder/'interface_configs.json'
 if check:
  if config.read_bytes()!=raw:raise RuntimeError('Pinned interface config differs')
 else:config.write_bytes(raw)
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 def source(name):p=SERVER/name;read(p);java.append(p)
 for name in ['core/game/component/ComponentDefinition.java','core/game/component/InterfaceType.java','core/game/component/CloseEvent.java','core/game/node/entity/player/info/ClientInfo.java','core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java','core/net/packet/Context.java','core/net/packet/OutgoingPacket.java','core/net/packet/out/WindowsPane.java','core/net/packet/out/Interface.java','core/net/packet/out/Config.java','core/net/packet/out/CloseInterface.java','core/net/packet/out/InterfaceConfig.java','core/net/packet/out/RunScriptPacket.java','core/net/packet/context/InterfaceConfigContext.java','core/net/packet/context/RunScriptContext.java','core/net/packet/context/WindowsPaneContext.java','core/net/packet/context/InterfaceContext.java','core/net/packet/context/ConfigContext.java']:source(name)
 component=read(SERVER/'core/game/component/Component.java');adapter('core/game/component/Component.java',component.replace('this.plugin = definition.getPlugin();','this.plugin = definition.getPlugin();oracle.Support.created(this);'))
 manager=read(SERVER/'core/game/node/entity/player/link/InterfaceManager.java');source('core/game/node/entity/player/link/InterfaceManager.java');methods=manager;constants=''
 login=read(SERVER/'core/game/node/entity/player/info/login/LoginConfiguration.java');adapter('core/game/node/entity/player/info/login/LoginConfiguration.java','package core.game.node.entity.player.info.login;import java.util.*;import core.plugin.Plugin;import core.game.node.entity.player.Player;import core.game.component.Component;public class LoginConfiguration{public static List<Plugin<Object>>LOGIN_PLUGINS=Collections.emptyList();public static void welcome(Player p){oracle.Support.emit("welcome");}public static void config(Player p){oracle.Support.configure();}'+c.method(login,'public static void sendGameConfiguration(final Player player)')+'}')
 names=set(re.findall(r'Components\.(\w+)',methods+constants+read(SERVER/'core/game/component/InterfaceType.java')));adapter('org/rs09/consts/Components.java','package org.rs09.consts;public class Components{'+''.join(f'public static final int {name}={int(name.rsplit("_",1)[1])};'for name in sorted(names))+'}')
 parser=read(SERVER/'core/game/system/config/InterfaceConfigParser.kt');adapter('core/game/system/config/InterfaceConfigParser.kt',parser.replace('var reader: FileReader? = null','var reader: java.io.Reader? = null').replace('FileReader(ServerConstants.CONFIG_PATH + "interface_configs.json")','java.io.StringReader(oracle.Support.config)'))
 adapter('core/ServerConstants.java','package core;public class ServerConstants{public static String CONFIG_PATH="";}');adapter('core/tools/SystemLogger.java','package core.tools;public class SystemLogger{}');adapter('core/tools/Log.java','package core.tools;public enum Log{WARN,FINE}')
 adapter('core/api/ContentAPI.kt','package core.api\nfun log(o:Any,l:core.tools.Log,s:String){oracle.Support.emit("log",s)}\nfun setVarp(p:core.game.node.entity.player.Player,id:Int,value:Int){oracle.Support.emit("varp",id,value);core.net.packet.PacketRepository.send(core.net.packet.out.Config::class.java,core.net.packet.context.ConfigContext(p,id,value))}')
 source('core/game/component/ComponentPlugin.java')
 adapter('core/game/interaction/InterfaceListeners.java','package core.game.interaction;import core.game.component.Component;import core.game.node.entity.player.Player;public class InterfaceListeners{public static boolean runOpen(Player p,Component c){oracle.Support.emit("listenerOpen",oracle.Support.slot(c));return false;}public static boolean runClose(Player p,Component c){oracle.Support.emit("listenerClose",oracle.Support.slot(c));return oracle.Support.bool("closeAllowed",true);}}')
 adapter('core/game/event/InterfaceOpenEvent.java','package core.game.event;public class InterfaceOpenEvent{public core.game.component.Component component;public InterfaceOpenEvent(core.game.component.Component c){component=c;}}')
 adapter('core/game/node/entity/combat/equipment/WeaponInterface.java','package core.game.node.entity.combat.equipment;public class WeaponInterface extends core.game.component.Component{public WeaponInterface(core.game.node.entity.player.Player p){super(92);oracle.Support.emit("weaponCreate",oracle.Support.slot(this));}public int getAutospellId(int id){oracle.Support.emit("autoId",oracle.Support.slot(this),id);return id+10;}public void selectAutoSpell(int id,boolean a,boolean b){oracle.Support.emit("autoSelect",oracle.Support.slot(this),id);}}')
 adapter('content/global/handlers/iface/FriendsListInterface.java','package content.global.handlers.iface;public class FriendsListInterface extends core.game.component.Component{public FriendsListInterface(){super(550);oracle.Support.emit("friendsCreate",oracle.Support.slot(this));}}')
 adapter('content/region/misc/tutisland/handlers/TutorialStage.java','package content.region.misc.tutisland.handlers;public class TutorialStage{public static void hideTabs(core.game.node.entity.player.Player p,boolean b){oracle.Support.emit("tutorialTabs");}}')
 adapter('core/cache/crypto/ISAACCipher.java','package core.cache.crypto;public class ISAACCipher{public int getNextValue(){return 0;}}');adapter('core/cache/misc/buffer/ByteBufferUtils.java','package core.cache.misc.buffer;public class ByteBufferUtils{public static String getString(java.nio.ByteBuffer b){throw new UnsupportedOperationException();}public static int packGJString2(int a,byte[]b,String s){throw new UnsupportedOperationException();}}')
 adapter('core/net/packet/PacketRepository.java','package core.net.packet;public class PacketRepository{public static void send(Class cls,Context c){try{((OutgoingPacket)cls.getDeclaredConstructor().newInstance()).send(c);}catch(RuntimeException e){throw e;}catch(Exception e){throw new RuntimeException(e);}}}')
 adapter('core/net/IoSession.java','package core.net;public class IoSession{public core.game.node.entity.player.info.ClientInfo info;public IoSession(int m){info=new core.game.node.entity.player.info.ClientInfo(0,m,0,0);}public core.game.node.entity.player.info.ClientInfo getClientInfo(){return info;}public IoSession getIsaacPair(){return this;}public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}public void write(core.net.packet.IoBuffer b){oracle.Support.sent(b);}}')
 adapter('core/game/node/entity/player/Player.java',read(HERE/'Player.java'))
 adapter('core/game/event/InterfaceCloseEvent.java','package core.game.event;public class InterfaceCloseEvent{public core.game.component.Component component;public InterfaceCloseEvent(core.game.component.Component c){component=c;}}')
 read(HERE/'ManagerSupport.java');java.append(HERE/'ManagerSupport.java')
 source('core/plugin/Plugin.java')
 read(HERE/'ObjectsSupport.java');java.append(HERE/'ObjectsSupport.java')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java]);cases=requests(raw.decode('utf-8'));p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:r=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if r.returncode:raise RuntimeError(r.stderr.decode(errors='replace'))
 rows=[json.loads(l)for l in r.stdout.splitlines()if l];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py');initial=json.loads(read(HERE/'initial.lock.json'));assert hashlib.sha256(b''.join(data.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256'];objects_initial=json.loads(read(HERE/'objects.lock.json'));assert hashlib.sha256(b''.join(data.splitlines(keepends=True)[:objects_initial['caseCount']])).hexdigest()==objects_initial['sha256'];provenance=dict(format=1,initialInterfaceBaseline=initial,typedObjectsBaseline=objects_initial,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),configSha256=hashlib.sha256(raw).hexdigest(),configRows=len(json.loads(raw)),sourceHashAlgorithm='sha256, CR removed',scope='Complete original InterfaceManager class including open/close/single-tab/overlay/tab removal/restoration/lookup; original full Component with non-observable constructor identity recorder, full ComponentPlugin and Plugin default methods, typed plugin/close aliases, reentry/retirement and setUnclosable; complete ComponentDefinition/InterfaceType/ClientInfo, InterfaceConfigParser with string-reader input, IoBuffer and WindowsPane/Interface/Config writers. Constant component IDs resolved from pinned symbolic suffixes. Player, component content/listeners/weapon/familiar/friends/tutorial/config services, ISAACzero, packet sink and empty login-plugin list are explicit. Not complete UI/classes/content/startup or live gameplay proof.')
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Interface fixtures/provenance differ')
  print(f'{len(rows)} original interface scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n');print(f'Generated {len(rows)} original interface scenarios, {provenance["configRows"]} pinned configs.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
