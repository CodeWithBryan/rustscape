"""Join unchanged InteractPlugin bodies to original movement and world scheduling."""
import argparse,importlib.util,itertools,os,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
BASE=ROOT/'oracle/player-walk'
sys.path.insert(0,str(BASE))
spec=importlib.util.spec_from_file_location('walk_compiler',BASE/'generate.py');walk=importlib.util.module_from_spec(spec);spec.loader.exec_module(walk)
FIXTURE='interaction-runtime';MAIN='oracle.RuntimeSupport'
SCOPE='Unchanged complete InteractPlugin, Option and OptionHandler joined with unchanged MovementPulse, DestinationFlag, Pulse/PulseManager/PulseRunner, SmartPathfinder, WalkingQueue and original Entity.tick, Node facing and Location algorithms. Canonical native movement and plain tasks use one world runner and retain immutable captures. Profile locks/zones/handlers/facing delivery/combat/logging are explicit reentrant service adapters; full registries, masks, content and live scene fanout remain separate. JVM default locale ROOT; logical JSON input round trips are checked.'
def op(action,**kw):return dict(op=action,**kw)
def requests():
 rows=[]
 def add(actions,**kw):rows.append(dict(actions=actions,scenery=kw.pop('scenery',{}),**kw))
 for walk_flag,walk_for,delayed,handled,locked,moving,zone in itertools.product([False,True],repeat=7):
  add([op('handle')]+[op('tick')for _ in range(10)],handlerConfig=dict(walk=walk_flag,walkFor=walk_for,delayed=delayed,handled=handled),interactionLocked=locked,locked=moving,interactionZone=zone)
 for shape,rotation,size in itertools.product([0,2,10,22],range(4),[[1,1],[2,3]]):
  for kind in ['handle','invalid']:
   add([op(kind)]+[op('tick')for _ in range(12)],scenery=dict(shape=shape,rotation=rotation,size=size))
 for config in [dict(walk=True),dict(walk=False,delayed=True),dict(walk=False,delayed=False,handled=False)]:
  for mutation in [op('optionHandler',handler=None),op('optionHandler',handler=2),op('optionReplace',name='Search',handler=2),op('scenerySet',active=False),op('sceneryReplace'),op('set',interactionLocked=True),op('set',interactionZone=True),op('checksClear'),op('managerReplace')]:
   add([op('handle'),mutation]+[op('tick')for _ in range(10)],handlerConfig=config)
 for phase in ['handlerWalk','handlerWalkFor','handlerDelayed','handlerName','interactionZone','handlerHandle','interactionDispatch','facePlayer']:
  for action in [op('fail'),op('optionHandler',handler=2),op('handle'),op('managerReplace')]:
   for config in [dict(walk=True),dict(walk=False,delayed=True),dict(walk=False,delayed=False)]:
    add([op('handle')]+[op('tick')for _ in range(10)],handlerConfig=config,interactionHooks=[dict(event=phase,actions=[action])])
 for handler in [None,1]:
  for bfs in [False,True]:
   add([op('request',kind='interact',running=True),op('handle')]+[op('tick')for _ in range(15)],handler=handler,bfs=bfs,scenery=dict(shape=0))
 for active in [False,True]:
  add([op('handle'),op('checksClear')]+[op('tick')for _ in range(3)],handlerConfig=dict(walk=False,delayed=True),scenery=dict(active=active))
 for delayed in [False,True]:
  add([op('handle'),op('handle'),op('tick'),op('tick')],handlerConfig=dict(walk=False,delayed=delayed,handled=False))
 return rows
def install(adapter,read,c,server,run,java,kt):
 read(pathlib.Path(__file__))
 for name in ['InteractPlugin','OptionHandler']:
  p=server/f'core/game/interaction/{name}.java';read(p);java.append(p)
 java[:]=[p for p in java if p.as_posix()!=str(BASE/'Support.java').replace('\\','/')and not p.as_posix().endswith('adapters/core/game/interaction/OptionHandler.java')]
 source=read(BASE/'Support.java')
 needle='new OptionHandler(v(a,"handler",1))';assert source.count(needle)==1
 adapter('oracle/Support.java',source.replace(needle,'new RuntimeSupport.Handler(v(a,"handler",1))'))
 p=run/'core/game/node/Node.java';text=p.read_text();source=read(server/'core/game/node/Node.java')
 adapter('core/game/node/Node.java',text[:-1]+c.method(source,'public Location getFaceLocation (Location fromLoc)')+'public int getId(){return 12348;}}')
 p=run/'core/game/node/entity/Entity.java';text=p.read_text()
 text=text.replace('public boolean isMovementLocked(){','public boolean isInteractionLocked(){return oracle.RuntimeSupport.interactionLocked();}public boolean isMovementLocked(){')
 text=text.replace('public boolean isRunning(){return running;}','public void stop(){oracle.RuntimeSupport.emit("combatStop");running=false;}public boolean isRunning(){return running;}')
 text=text.replace('public static class Zone{','public static class Zone{public boolean interact(Node n,core.game.interaction.Option o){return oracle.RuntimeSupport.zone(n,o);}')
 adapter('core/game/node/entity/Entity.java',text)
 p=run/'core/game/node/entity/player/Player.java';text=p.read_text()
 text=text.replace('public void faceLocation(core.game.world.map.Location l){oracle.Support.emit("faceLocation",name);}', 'public void faceLocation(core.game.world.map.Location l){oracle.RuntimeSupport.face(l);}')
 text=text.replace('public static class Definition{','public void dispatch(core.game.event.InteractionEvent e){oracle.RuntimeSupport.emit("interactionDispatch",e.node==null?null:e.node.name,e.option);}public static class Definition{')
 adapter('core/game/node/entity/player/Player.java',text)
 for name in ['ItemDefinition','NPCDefinition','SceneryDefinition']:
  adapter('core/cache/def/impl/'+name+'.java','package core.cache.def.impl;public class '+name+'{public static core.game.interaction.OptionHandler getOptionHandler(int id,String s){return null;}public static '+name+' forId(int id){return new '+name+'();}public String[]getOptions(){return new String[5];}public String[]getGroundOptions(){return getOptions();}public String[]getInventoryOptions(){return getOptions();}public int[]getChildrenIds(){return null;}}')
 for name,path,definition in [('NPC','entity/npc','NPCDefinition'),('Scenery','scenery','SceneryDefinition')]:
  p=run/f'core/game/node/{path}/{name}.java';text=p.read_text()
  if name=='NPC':text=text[:-1]+'public core.cache.def.impl.NPCDefinition getDefinition(){return new core.cache.def.impl.NPCDefinition();}}'
  else:text=text.replace('public int getWalkingFlag(){','public String[]getOptions(){return new String[5];}public int getWalkingFlag(){')
  adapter(f'core/game/node/{path}/{name}.java',text)
 adapter('core/game/node/item/Item.java','package core.game.node.item;public class Item extends core.game.node.Node{public Item(int id){}public core.cache.def.impl.ItemDefinition getDefinition(){return new core.cache.def.impl.ItemDefinition();}}')
 adapter('core/game/event/InteractionEvent.java','package core.game.event;public class InteractionEvent{public core.game.node.Node node;public String option;public InteractionEvent(core.game.node.Node n,String o){node=n;option=o;}}')
 adapter('core/game/interaction/IntType.java','package core.game.interaction;public enum IntType{ITEM;}')
 adapter('core/game/interaction/InteractionListeners.java','package core.game.interaction;public class InteractionListeners{public static boolean run(int id,IntType t,String s,core.game.node.entity.player.Player p,core.game.node.Node n){return false;}}')
 adapter('core/game/container/Container.java','package core.game.container;public class Container{}')
 adapter('core/plugin/Plugin.java','package core.plugin;public interface Plugin<T>{Plugin<T>newInstance(T t)throws Throwable;Object fireEvent(String id,Object...args);}')
 adapter('core/net/packet/context/InteractionOptionContext.java','package core.net.packet.context;public class InteractionOptionContext{public InteractionOptionContext(core.game.node.entity.player.Player p,int i,String s){}}')
 adapter('core/net/packet/out/InteractionOption.java','package core.net.packet.out;public class InteractionOption{}')
 p=run/'core/net/packet/PacketRepository.java';text=p.read_text();adapter('core/net/packet/PacketRepository.java',text[:-1]+'public static void send(Class c,core.net.packet.context.InteractionOptionContext p){}'+'}')
 read(HERE/'RuntimeSupport.java');java.append(HERE/'RuntimeSupport.java')
def generate(check):
 walk.RUN=ROOT/'runs/interaction-runtime-oracle'
 walk.generate(check,sys.modules[__name__])
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();walk.c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
