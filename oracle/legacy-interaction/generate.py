"""Unchanged InteractPlugin/Option/OptionHandler with mutable orchestration services."""
import argparse,hashlib,importlib.util,itertools,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
RUN=ROOT/'runs/legacy-interaction-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
read,run=c.read,c.run
def op(operation,**kw):return dict(op=operation,**kw)
def requests():
 rows=[]
 def add(actions=None,**kw):rows.append(dict(actions=actions or [op('handle'),op('arrival')],**kw))
 for h,locked,moving,zone,walk,walk_for,delayed,handled in itertools.product([None,1],[False,True],[False,True],[False,True],[False,True],[False,True],[False,True],[False,True]):
  add(handler=h,locked=locked,moving=moving,zone=zone,handlers={'1':dict(walk=walk,walkFor=walk_for,delayed=delayed,handled=handled)})
 for action in ['handle','invalid','walk','default']:
  for target,player,location,manager,name in itertools.product([None,2],[None,1],[None,[3211,3217,0]],[None,0],[None,'Open','İΣ😀']):
   add([op(action),op('arrival')],target=target,player=player,location=location,manager=manager,name=name)
 events=['locks','interactionLocked','movementLocked','location','debug','walk','walkFor','delayed','manager','construct','run','clear','unhandled','zone','properties','combat','combatStop','faceLocation','facePlayer','message','handle','dispatch','exception','errorMessage','log']
 mutations=[op('fail'),op('set',manager=1,handler=2,locked=True),op('set',handler=None,zone=True),
            op('replace',name='Search',index=4,handler=2),op('set',properties=None),
            op('handle')]
 for event,mutation in itertools.product(events,mutations):
  for mode in ['walk','default']:
   add([op('handle'),op('arrival'),op('invalid'),op('arrival')],
       handlers={'1':dict(walk=mode=='walk',delayed=False,handled=False)},
       hooks=[dict(event=event,actions=[mutation])])
 for event in ['zone','properties','construct','interactionLocked']:
  add([op('handle'),op('arrival')],handlers={'1':dict(walk=False,delayed=True)},hooks=[dict(event=event,actions=[op('fail')])])
 add([op('handle'),op('replace',name='Search',index=4,handler=2),op('arrival'),op('handle'),op('arrival',index=0)])
 add([op('handle'),op('set',handler=None),op('arrival')])
 add([op('handle'),op('checksClear'),op('arrival')])
 add([op('handle'),op('arrival')],hooks=[dict(event='construct',actions=[op('set',manager=1),op('arrival')])])
 for event in ['debug','facePlayer','handle']:
  add([op('handle'),op('arrival')],derived=True,hooks=[dict(event=event,actions=[op('fail')])])
 for action,player in itertools.product(['handle','invalid','walk','default'],[None,1]):
  add([op(action,nullOption=True),op('arrival')],player=player)
 for message in [None,'changed after print']:
  add(hooks=[dict(event='debug',actions=[op('fail')]),dict(event='exception',actions=[op('set',errorMessage=message)])])
 rng=random.Random(530);edges=[-2147483648,-32769,-16384,-1,0,1,2,3,255,16383,32768,2147483647]
 for _ in range(4096):
  x,y,z,size,fx,fy,fz=rng.choice(edges),rng.choice(edges),rng.randrange(-4,8),rng.choice(edges),rng.choice(edges),rng.choice(edges),rng.randrange(-4,8)
  add([op('face')],location=[x,y,z],size=size,playerLocation=[fx,fy,fz])
 return rows
def generate(check, extension=None):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
 run(['git','-C',ref,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'))
 pin=json.loads(read(ROOT/'oracle/auth/json.lock.json'));jar=RUN/'json-simple-1.1.1.jar'
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=pin['sha256']:
  with urllib.request.urlopen(pin['url'],timeout=60)as response:data=response.read()
  if len(data)!=pin['size']or hashlib.sha256(data).hexdigest()!=pin['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  if extension is not None:text=extension.transform(name,text)
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 for name in ['core/game/interaction/InteractPlugin.java','core/game/interaction/Option.java','core/game/interaction/OptionHandler.java','core/api/utils/Vector.kt']:
  p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
 location=read(SERVER/'core/game/world/map/Location.kt')
 adapter('core/game/world/map/Location.kt','''package core.game.world.map
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;
'''+next(line.strip()for line in location.splitlines()if'fun transform(diffX:'in line)+'''
companion object{@JvmStatic @JvmOverloads fun create(x:Int,y:Int,z:Int=0)=Location(x,y,z)};@JvmInline '''+c.method(location,'value class LocationHash(')+'}')
 adapter('core/game/world/map/Direction.java','package core.game.world.map;public enum Direction{NORTH_WEST,NORTH,NORTH_EAST,WEST,EAST,SOUTH_WEST,SOUTH,SOUTH_EAST;}')
 node=read(SERVER/'core/game/node/Node.java')
 adapter('core/game/node/Node.java','''package core.game.node;import core.api.utils.Vector;import core.game.world.map.Location;
public class Node{public int key,size=1;public Location location;public int getId(){return key;}public Location getLocation(){oracle.Support.emit("location",key);return location;}
'''+c.method(node,'public Vector getMathematicalCenter()')+c.method(node,'public Location getFaceLocation (Location fromLoc)').replace('{','{oracle.Support.emit("faceLocation",key,oracle.Support.coord(fromLoc));',1)+'}')
 adapter('core/game/node/entity/player/Player.java','''package core.game.node.entity.player;import core.game.node.Node;import core.game.world.map.Location;import core.game.event.InteractionEvent;
public class Player extends Node{public Integer manager=0,properties=0;public Locks getLocks(){oracle.Support.emit("locks");return new Locks();}public class Locks{public boolean isInteractionLocked(){oracle.Support.emit("interactionLocked");return oracle.Support.answer("locked");}public boolean isMovementLocked(){oracle.Support.emit("movementLocked");return oracle.Support.answer("moving");}}
public oracle.Support.Manager getPulseManager(){oracle.Support.emit("manager");return manager==null?null:oracle.Support.managers[manager];}
public void debug(String s){oracle.Support.emit("debug",s);}public void dispatch(InteractionEvent e){oracle.Support.emit("dispatch",e.node==null?null:e.node.key,e.option);}
public void faceLocation(Location l){oracle.Support.emit("facePlayer",oracle.Support.coord(l));}public Dispatch getPacketDispatch(){return new Dispatch();}public class Dispatch{public void sendMessage(String s){oracle.Support.emit("message",s);}}
public Props getProperties(){oracle.Support.emit("properties");return properties==null?null:new Props(properties);}public static class Props{int key;Props(int k){key=k;}public Combat getCombatPulse(){oracle.Support.emit("combat",key);return new Combat(key);}}public static class Combat{int key;Combat(int k){key=k;}public void stop(){oracle.Support.emit("combatStop",key);}}
public Zone getZoneMonitor(){return new Zone();}public class Zone{public boolean interact(Node n,core.game.interaction.Option o){oracle.Support.emit("zone",n==null?null:n.key,oracle.Support.optionKey(o));return oracle.Support.answer("zone");}}}''')
 adapter('core/game/system/task/Pulse.java','''package core.game.system.task;import core.game.node.Node;
public abstract class Pulse{public int id,delay;public Node[]checks;public Pulse(int d,Node...n){delay=d;checks=n;oracle.Support.construct(this);}public abstract boolean pulse();}''')
 adapter('core/game/interaction/MovementPulse.java','''package core.game.interaction;import core.game.node.Node;import core.game.node.entity.player.Player;
public abstract class MovementPulse extends core.game.system.task.Pulse{public OptionHandler routing;public MovementPulse(Player p,Node n){super(1,p,n);}public MovementPulse(Player p,Node n,OptionHandler h){super(1,p,n);routing=h;}}''')
 for name in ['ItemDefinition','NPCDefinition','SceneryDefinition']:
  adapter('core/cache/def/impl/'+name+'.java','''package core.cache.def.impl;public class '''+name+'''{public static core.game.interaction.OptionHandler getOptionHandler(int id,String name){return null;}public static '''+name+''' forId(int id){return new '''+name+'''();}public String[]getOptions(){return new String[5];}public String[]getGroundOptions(){return getOptions();}public String[]getInventoryOptions(){return getOptions();}public int[]getChildrenIds(){return null;}}''')
 for kind,package,definition in [('NPC','entity/npc','NPCDefinition'),('Scenery','scenery','SceneryDefinition'),('Item','item','ItemDefinition')]:
  adapter('core/game/node/'+package+'/'+kind+'.java','package core.game.node.'+package.replace('/','.')+';public class '+kind+' extends core.game.node.Node{public core.cache.def.impl.'+definition+' getDefinition(){return new core.cache.def.impl.'+definition+'();}}')
 adapter('core/game/event/InteractionEvent.java','package core.game.event;public class InteractionEvent{public core.game.node.Node node;public String option;public InteractionEvent(core.game.node.Node n,String o){node=n;option=o;}}')
 adapter('core/game/node/entity/impl/PulseType.java','package core.game.node.entity.impl;public enum PulseType{STANDARD;}')
 adapter('core/game/container/Container.java','package core.game.container;public class Container{}')
 adapter('core/game/world/GameWorld.java','package core.game.world;public class GameWorld{public static Pulser getPulser(){return new Pulser();}public static class Pulser{public void submit(core.game.system.task.Pulse p){throw new UnsupportedOperationException();}}}')
 adapter('core/game/interaction/IntType.java','package core.game.interaction;public enum IntType{ITEM;}')
 adapter('core/game/interaction/InteractionListeners.java','package core.game.interaction;public class InteractionListeners{public static boolean run(int id,IntType type,String name,core.game.node.entity.player.Player p,core.game.node.Node n){return false;}}')
 adapter('core/plugin/Plugin.java','package core.plugin;public interface Plugin<T>{Plugin<T>newInstance(T arg)throws Throwable;Object fireEvent(String id,Object...args);}')
 adapter('core/tools/Log.java','package core.tools;public enum Log{ERR;}')
 adapter('core/tools/SystemLogger.java','package core.tools;public class SystemLogger{}')
 adapter('core/api/ContentAPIKt.java','package core.api;public class ContentAPIKt{public static void log(Class<?>c,core.tools.Log level,String message){oracle.Support.emit("log",c.getName(),message);}}')
 adapter('core/net/packet/context/InteractionOptionContext.java','package core.net.packet.context;public class InteractionOptionContext{public InteractionOptionContext(core.game.node.entity.player.Player p,int i,String s){}}')
 adapter('core/net/packet/out/InteractionOption.java','package core.net.packet.out;public class InteractionOption{}')
 adapter('core/net/packet/PacketRepository.java','package core.net.packet;public class PacketRepository{public static void send(Class<?>c,Object context){}}')
 if extension is not None:extension.register(adapter,read,c)
 support=HERE/'Support.java' if extension is None else extension.HERE/'Support.java'
 read(support);java.append(support);cp=os.pathsep.join(map(str,[classes,std,jar]))
 run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',os.pathsep.join(map(str,[std,jar])),'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();path=RUN/'input.jsonl';path.write_text(''.join(json.dumps(row)+'\n'for row in cases),encoding='utf-8')
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with path.open('rb')as stdin:r=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=stdin,capture_output=True,timeout=120)
 if r.returncode:raise RuntimeError(r.stderr.decode(errors='replace'))
 rows=[json.loads(line)for line in r.stdout.splitlines()if line];assert len(rows)==len(cases)
 assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases)),'Oracle transport changed a scenario'
 data=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Unchanged complete InteractPlugin, Option and OptionHandler classes; selected handle/invalid/walk/default methods and actual anonymous pulse bodies, plus original Node center/face and Location/Vector geometry. Mutable recording managers/constructors, locks, zones, handler APIs, combat, messages, dispatch and exception/log services. Default locale explicitly ROOT. Item actions, option table/default initialization, actual manager/movement scheduling, real facing masks, content/world and live delivery remain separate.')
 if extension is not None:provenance['scope']=extension.SCOPE
 folder=ROOT/'fixtures'/('legacy-interaction' if extension is None else extension.FOLDER)
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Legacy interaction fixture/provenance differs')
  print(f'{len(rows)} original '+('legacy-interaction'if extension is None else extension.FOLDER)+' scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original '+('legacy-interaction'if extension is None else extension.FOLDER)+' scenarios.')
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');parser.add_argument('--java-home',type=pathlib.Path);args=parser.parse_args();c.JAVA_HOME=args.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(args.check)
