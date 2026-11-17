"""Unchanged ground click method over original manager/chunk/ground classes."""
import argparse,importlib.util,itertools,os,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('spawn',ROOT/'oracle/ground-spawn/generate.py');spawn=importlib.util.module_from_spec(spec);spec.loader.exec_module(spawn)
SCOPE='Unchanged PacketProcessor.processGroundItemAction joined to complete original GroundItemManager/GroundItem/GroundSpawn/PulseRunner and chunk/packet bodies. Actual identity/privacy/owner/removed lookup precedes scripts; recording retained interactions/options, plugin/listener registries, invalid/approach and minimap services expose order, late reads, short circuits, reentrancy and failed prefixes. Native replay likewise uses actual native manager/canonical actors/chunks. Full default registry, ground target navigation/arrival, inventory/content/profile and live authoritative Take remain separate.'
def transform(name,text,source,c):
 text=spawn.transform(name,text,source,c)
 if name=='oracle/Support.java':
  text=text.replace('SpawnSupport.reset();','SpawnSupport.reset();GroundActionSupport.reset();')
  text=text.replace('ManagerSupport.snapshot(s);','ManagerSupport.snapshot(s);if(Boolean.TRUE.equals(input.get("groundAction")))GroundActionSupport.snapshot(s);')
 elif name=='oracle/SpawnSupport.java':text=text.replace('default:throw new IllegalArgumentException((String)a.get("op"));','default:return GroundActionSupport.act(a);')
 elif name=='core/game/node/item/Item.java':
  text=text.rstrip()[:-1]+'''public Integer interactionKey=0;public core.game.interaction.InteractPlugin getInteraction(){oracle.Support.emit("interaction",oracle.ManagerSupport.id((GroundItem)this));return interactionKey==null?null:oracle.GroundActionSupport.interactions[interactionKey];}}'''
 elif name=='core/game/node/entity/player/Player.java':
  text=text.replace('public Location location;','protected Location location;public Location rawLocation(){return location;}public void rawLocation(Location l){location=l;}')
  text=text.rstrip()[:-1]+'''public Scripts getScripts(){return new Scripts(id);}public static class Scripts{int id;Scripts(int i){id=i;}public void reset(){oracle.Support.emit("scripts",id);}}}'''
 elif name=='oracle/ManagerSupport.java':
  text=text.replace('p.location=Support.loc(a.get("location"));','p.rawLocation(Support.loc(a.get("location")));').replace('Support.coord(p.location)','Support.coord(p.rawLocation())')
 return text
def register(adapter,source,read,c):
 spawn.register(adapter,source,read,c)
 body=c.method(source('core/net/packet/PacketProcessor.kt')[1],'private fun processGroundItemAction(pkt: Packet.GroundItemAction)')
 adapter('core/net/packet/PacketProcessor.kt','''package core.net.packet
import core.game.node.entity.player.Player
import core.game.node.item.GroundItemManager
import core.game.world.map.Location
import core.game.interaction.*
import core.net.packet.`in`.Packet
object PacketProcessor{
@JvmStatic fun handle(pkt:Packet.GroundItemAction){processGroundItemAction(pkt)}
fun sendClearMinimap(player:Player){oracle.Support.emit("minimap",player.id)}
'''+body+'\n}')
 adapter('core/net/packet/in/Packet.java','package core.net.packet.in;public class Packet{public static class GroundItemAction{public core.game.node.entity.player.Player player;public int x,y,id,optIndex;}}')
 adapter('core/game/interaction/Option.java','''package core.game.interaction;public final class Option{public static Option NULL=new Option("null",0,2);private final String name;private final int index;public final int key;public Option(String n,int i,int k){name=n;index=i;key=k;}public String getName(){return name;}public int getIndex(){return index;}}''')
 adapter('core/game/interaction/InteractPlugin.java','''package core.game.interaction;import core.game.node.entity.player.Player;import core.game.node.item.GroundItem;
 public class InteractPlugin{public int key;public Option[]options;public InteractPlugin(int k){key=k;options=new Option[5];}public Option get(int i){oracle.Support.emit("option",key,i);return options[i];}
 public void handle(Player p,Option o){oracle.Support.emit("handleGround",key,p.id,o.key);}public static void handleInvalidInteraction(Player p,GroundItem g,Option o){oracle.Support.emit("invalidGround",p.id,oracle.ManagerSupport.id(g),o.key);}}''')
 adapter('core/game/interaction/IntType.java','package core.game.interaction;public enum IntType{GROUNDITEM,ITEM;}')
 adapter('core/game/interaction/PluginInteractionManager.java','''package core.game.interaction;import core.game.node.entity.player.Player;import core.game.node.item.GroundItem;public class PluginInteractionManager{public static boolean handle(Player p,GroundItem g,Option o){boolean v=oracle.GroundActionSupport.plugin;oracle.Support.emit("pluginInteraction",p.id,oracle.ManagerSupport.id(g),o.key);return v;}}''')
 adapter('core/game/interaction/InteractionListeners.java','''package core.game.interaction;import core.game.node.entity.player.Player;import core.game.node.item.GroundItem;public class InteractionListeners{public static boolean run(int id,IntType t,String name,Player p,GroundItem g){boolean v=t==IntType.GROUNDITEM?oracle.GroundActionSupport.ground:oracle.GroundActionSupport.item;oracle.Support.emit(t==IntType.GROUNDITEM?"groundListener":"itemListener",id,name,p.id,oracle.ManagerSupport.id(g));return v;}}''')
 adapter('oracle/GroundActionSupport.java',read(HERE/'GroundActionSupport.java'));read(pathlib.Path(__file__))
def requests():
 rows=[]
 def a(op,**kw):return dict(op=op,**kw)
 def setup(**kw):return [a('newGround',location=[3207,3216,0],**kw),a('managerCreate',g=0)]
 def add(actions,**kw):rows.append(dict(manager=True,spawns=True,groundAction=True,actions=actions,**kw))
 for owner,viewer,index in itertools.product([None,0,1],[None,0,1],[-1,0,1,2,3,4,5,2147483647]):
  add(setup(p=owner)+[a('groundProcess',p=viewer,index=index)])
 for plugin,ground,item,interaction in itertools.product([False,True],[False,True],[False,True],[None,0,1]):
  add(setup()+[a('groundActionSet',plugin=plugin,ground=ground,item=item,interaction=interaction,g=0),a('groundProcess')])
 for removed,visible,tick,id in itertools.product([False,True],[False,True],[0,100,200],[1,2]):
  add(setup(p=1)+[a('setGround',g=0,removed=removed,forceVisible=visible),a('tick',value=tick),a('groundProcess',id=id)])
 for location in [None,[3207,3216,1],[3207,3216,3],[0,0,0]]:
  add(setup()+[a('playerState',location=location),a('groundProcess')])
 events=['location','lookup','scripts','interaction','option','pluginInteraction','groundListener','itemListener','handleGround','invalidGround','minimap']
 mutations=[a('fail'),a('groundActionSet',g=0,interaction=None),a('groundActionSet',g=0,interaction=1),
  a('groundActionSet',plugin=True,ground=True,item=True),a('groundActionSet',table=0,options=[1,None,None,None,None]),
  a('setItem',g=0,id=99),a('setGround',g=0,removed=True,location=None),a('managerDestroy',g=0),a('groundProcess')]
 for event,at,mutation in itertools.product(events,[1,2],mutations):
  add(setup()+[a('groundProcess',index=0 if event=='invalidGround' else 2),a('groundProcess')],hooks=[dict(event=event,at=at,actions=[mutation])])
 add(setup()+[a('newGround',location=[3207,3216,0]),a('managerCreate',g=1),a('groundProcess'),a('setGround',g=0,removed=True),a('groundProcess')])
 return rows
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args()
 spawn.manager.base.c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None)
 spawn.manager.base.generate(a.check,sys.modules[__name__],'ground-action')
