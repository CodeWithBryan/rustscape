// Recording stand-ins for the systems the unmodified dialogue DSL classes reach.
// generate.py splits this bundle at each "//// path" marker into one Java file
// and replaces each /* ORIGINAL_* */ marker with original text read from the
// pinned reference at generation time (Entity's hook members and
// Component.setUnclosable). Nothing else here is a port of the class it names:
// every method records its call through oracle.Rec and answers from the case.
//// oracle/Rec.java
package oracle;
import java.util.*;
public final class Rec {
 public static List<Object> events=new ArrayList<>();public static int count,failAt;
 public static final class Failure extends RuntimeException{}
 public static int n(Object o){return((Number)o).intValue();}
 /** Text as a JSON string, or as its UTF-16 code units when it holds a surrogate. */
 public static Object units(String s){if(s==null)return null;boolean plain=true;for(char c:s.toCharArray())plain&=!Character.isSurrogate(c);if(plain)return s;List<Integer> out=new ArrayList<>();for(char c:s.toCharArray())out.add((int)c);return out;}
 /** Records one call; the failAt-th call of the swept action throws. */
 public static void emit(String phase,Object...args){
  List<Object> row=new ArrayList<>();row.add(phase);row.addAll(Arrays.asList(args));events.add(row);
  count++;if(failAt>0&&count==failAt)throw new Failure();
 }
}
//// core/game/node/Node.java
package core.game.node;
public abstract class Node { public core.game.node.entity.npc.NPC asNpc(){return null;} }
//// core/game/world/map/Location.java
package core.game.world.map;
public class Location extends core.game.node.Node { public final int x,y,z;public Location(int x,int y,int z){this.x=x;this.y=y;this.z=z;} }
//// core/game/node/entity/Entity.java
package core.game.node.entity;
import java.util.*;import oracle.Rec;import core.game.event.Event;import core.game.event.EventHook;import core.game.world.map.Location;
public abstract class Entity extends core.game.node.Node {
 /* ORIGINAL_HOOKS_FIELD */
 public abstract int getId();
 public abstract int oid();
 public Map<Class<?>,ArrayList<EventHook>> rawHooks(){return hooks;}
 public void faceLocation(Location l){Rec.emit("faceLocation",oid(),l==null?null:Arrays.asList(l.x,l.y,l.z));}
 public boolean face(Entity e){Rec.emit("face",oid());return true;}
 public void faceTemporary(Entity e,int ticks){Rec.emit("faceTemporary",oid(),ticks);}
 /* ORIGINAL_ENTITY_DISPATCH */
 /* ORIGINAL_ENTITY_UNHOOK */
 /* ORIGINAL_ENTITY_HOOK_EVENT */
 /* ORIGINAL_ENTITY_HOOK_CLASS */
}
//// core/game/node/entity/npc/NPC.java
package core.game.node.entity.npc;
import oracle.Rec;import core.game.node.entity.player.Player;
public class NPC extends core.game.node.entity.Entity {
 public static final class Queue{final int oid;Queue(int o){oid=o;}public void reset(){Rec.emit("walkReset",oid);}}
 public static final class Pulses{final int oid;Pulses(int o){oid=o;}public void clear(){Rec.emit("pulseClear",oid);}}
 public final int oid;public int rawId;public NPC rawShown;public Player rawDialoguePlayer;
 public NPC(int oid,int id){this.oid=oid;this.rawId=id;this.rawShown=this;}
 /** {@code new NPC(id)}: joins the scenario's NPC list. */
 public NPC(int id){this(oracle.Oracle.npcs.size(),id);oracle.Oracle.npcs.add(this);Rec.emit("newNpc",oid,id);}
 public int oid(){return oid;}
 public int getId(){Rec.emit("npcId",oid);return rawId;}
 public void setDialoguePlayer(Player p){Rec.emit("dialoguePlayer",oid,p==null?null:p.oid);rawDialoguePlayer=p;}
 public Queue getWalkingQueue(){Rec.emit("walkingQueue",oid);return new Queue(oid);}
 public Pulses getPulseManager(){Rec.emit("pulseManager",oid);return new Pulses(oid);}
 public NPC getShownNPC(Player p){Rec.emit("shownNpc",oid,p==null?null:p.oid);return rawShown;}
}
//// core/game/container/Container.java
package core.game.container;
import oracle.Rec;import core.game.node.item.Item;
public class Container {
 final int oid;public Container(int oid){this.oid=oid;}
 public int getAmount(Item item){Rec.emit("invAmount",oid,item==null?null:item.ref);return item==null?0:oracle.Oracle.inventory(item.getId());}
}
//// core/game/node/entity/player/link/QuestRepository.java
package core.game.node.entity.player.link;
import oracle.Rec;import content.data.Quests;
public class QuestRepository {
 final int oid;public QuestRepository(int oid){this.oid=oid;}
 public int getStage(Quests quest){Rec.emit("questStage",oid,quest.name());return oracle.Oracle.quest(quest.name());}
}
//// core/game/node/entity/player/Player.java
package core.game.node.entity.player;
import oracle.Rec;import java.util.*;
import core.game.node.entity.player.link.*;import core.game.dialogue.DialogueInterpreter;import core.game.event.Event;import core.game.event.EventHook;import core.game.world.map.Location;
public class Player extends core.game.node.entity.Entity {
 public final int oid;private PacketDispatch dispatch;private InterfaceManager manager;private DialogueInterpreter interpreter;
 private String username;private boolean male=true;private final Map<String,Object> attributes=new TreeMap<>();
 public Location rawLocation=new Location(3222,3218,0);
 private final core.game.container.Container inventory;private final QuestRepository quests;
 public Player(int oid){this.oid=oid;this.username="Player"+oid;inventory=new core.game.container.Container(oid);quests=new QuestRepository(oid);}
 public void raw(PacketDispatch d,InterfaceManager m,DialogueInterpreter i){dispatch=d;manager=m;interpreter=i;}
 public void rawMale(boolean b){male=b;}
 public Map<String,Object> rawAttributes(){return attributes;}
 public int oid(){return oid;}
 public int getId(){Rec.emit("playerId",oid);return 0;}
 public PacketDispatch getPacketDispatch(){Rec.emit("dispatch",oid);return dispatch;}
 public InterfaceManager getInterfaceManager(){Rec.emit("manager",oid);return manager;}
 public DialogueInterpreter getDialogueInterpreter(){Rec.emit("interpreter",oid);return interpreter;}
 public String getUsername(){Rec.emit("username",oid);return username;}
 public boolean isMale(){Rec.emit("male",oid);return male;}
 public boolean isDebug(){Rec.emit("isDebug",oid);return false;}
 public Location getLocation(){Rec.emit("location",oid);return rawLocation;}
 public core.game.container.Container getInventory(){return inventory;}
 public QuestRepository getQuestRepository(){return quests;}
 public void sendMessage(String s){Rec.emit("message",oid,Rec.units(s));}
 public void debug(String s){Rec.emit("debug",oid,Rec.units(s));}
 public void setAttribute(String key,Object value){Rec.emit("setAttribute",oid,key,oracle.Describe.value(value));attributes.put(key,value);}
 @SuppressWarnings("unchecked") public <T> T getAttribute(String key,T fail){Rec.emit("getAttribute",oid,key);Object v=attributes.get(key);return v==null?fail:(T)v;}
 @Override public void dispatch(Event e){Rec.emit("event",oid,oracle.Describe.event(e));super.dispatch(e);}
 @Override public void hook(Class<?> event,EventHook hook){Rec.emit("hook",oid,event.getSimpleName());super.hook(event,hook);}
 @Override public void unhook(EventHook hook){Rec.emit("unhook",oid);super.unhook(hook);}
}
//// core/game/node/entity/player/link/PacketDispatch.java
package core.game.node.entity.player.link;
import oracle.Rec;import java.util.*;
public class PacketDispatch {
 public final int oid;public PacketDispatch(int oid){this.oid=oid;}
 public void sendString(String s,int iface,int child){Rec.emit("text",oid,Rec.units(s),iface,child);}
 public void sendInterfaceConfig(int iface,int child,boolean hidden){Rec.emit("hidden",oid,iface,child,hidden);}
 public void sendAnimationInterface(int anim,int iface,int child){Rec.emit("anim",oid,anim,iface,child);}
 public void sendItemOnInterface(int item,int amount,int iface,int child){Rec.emit("itemOn",oid,item,amount,iface,child);}
 public void sendNpcOnInterface(int npc,int iface,int child){Rec.emit("npcOn",oid,npc,iface,child);}
 public void sendPlayerOnInterface(int iface,int child){Rec.emit("playerOn",oid,iface,child);}
 public void sendRepositionOnInterface(int iface,int child,int x,int y){Rec.emit("reposition",oid,iface,child,x,y);}
 public void sendAngleOnInterface(int iface,int child,int zoom,int pitch,int yaw){Rec.emit("angle",oid,iface,child,zoom,pitch,yaw);}
 public void sendItemZoomOnInterface(int item,int zoom,int iface,int child){Rec.emit("itemZoom",oid,item,zoom,iface,child);}
 public void sendRunScript(int id,String types,Object...objects){List<Object> out=new ArrayList<>();for(Object o:objects)out.add(oracle.Describe.value(o));Rec.emit("runScript",oid,id,types,out);}
 public void sendMessage(String s){Rec.emit("dmessage",oid,Rec.units(s));}
}
//// core/game/node/entity/player/link/InterfaceManager.java
package core.game.node.entity.player.link;
import oracle.Rec;import core.game.component.Component;
public class InterfaceManager {
 public final int oid;private Component chatbox;public final Component base;
 public InterfaceManager(int oid){this.oid=oid;this.base=new Component(137);this.chatbox=base;}
 public Component rawChatbox(){return chatbox;}
 public Component openChatbox(int iface){chatbox=new Component(iface);Rec.emit("open",oid,iface,chatbox.ref);return chatbox;}
 public boolean closeChatbox(){Rec.emit("closeChatbox",oid);chatbox=base;return true;}
 public Component getChatbox(){Rec.emit("chatbox",oid);return chatbox;}
}
//// core/game/component/Component.java
package core.game.component;
import oracle.Rec;import java.util.*;import core.game.node.entity.player.Player;
public class Component {
 public static List<Component> all=new ArrayList<>();
 public final int ref;private int id;private CloseEvent closeEvent;
 public Component(int id){this.id=id;this.ref=all.size();all.add(this);}
 public int rawId(){return id;}public CloseEvent rawCloseEvent(){return closeEvent;}
 public int getId(){Rec.emit("id",ref);return id;}
 public CloseEvent getCloseEvent(){Rec.emit("closeEvent",ref);return closeEvent;}
 public void setCloseEvent(CloseEvent e){Rec.emit("setCloseEvent",ref);closeEvent=e;}
 /* ORIGINAL_SET_UNCLOSABLE */
}
//// core/game/node/item/Item.java
package core.game.node.item;
import oracle.Rec;
public class Item {
 public static java.util.List<Item> all=new java.util.ArrayList<>();
 public final int ref;public int rawId,rawAmount;public String rawName;
 public Item(int id){this(id,1);}
 public Item(int id,int amount){rawId=id;rawAmount=amount;rawName="Item "+id;ref=all.size();all.add(this);Rec.emit("newItem",ref,id,amount);}
 public int getId(){return rawId;}public int getAmount(){return rawAmount;}
 public String getName(){Rec.emit("itemName",ref);return rawName;}
}
//// core/cache/def/impl/ItemDefinition.java
package core.cache.def.impl;
import oracle.Rec;import java.util.*;
public class ItemDefinition {
 public static Map<Integer,ItemDefinition> all=new TreeMap<>();
 public final int id;public int zoom,rotationX,rotationY;public String name;
 public final Map<String,Object> handlerMap;
 public ItemDefinition(int id){this.id=id;this.zoom=2000+id;this.rotationX=id*3;this.rotationY=id*7;this.name="Def "+id;
  handlerMap=new HashMap<String,Object>(){@Override public Object put(String k,Object v){Rec.emit("handlerPut",id,k,oracle.Describe.value(v));return super.put(k,v);}};}
 public static ItemDefinition forId(int id){Rec.emit("itemDef",id);return all.computeIfAbsent(id,ItemDefinition::new);}
 public int getModelZoom(){return zoom;}public int getModelRotationX(){return rotationX;}public int getModelRotationY(){return rotationY;}public String getName(){return name;}
 public Map<String,Object> getHandlers(){return handlerMap;}
 @SuppressWarnings("unchecked") public <T> T getConfiguration(String key,T fail){return fail;}
}
//// core/cache/def/impl/NPCDefinition.java
package core.cache.def.impl;
import oracle.Rec;
public class NPCDefinition {
 private final String name;private NPCDefinition(String n){name=n;}
 public static NPCDefinition forId(int id){Rec.emit("npcDef",id);return new NPCDefinition("Npc "+id);}
 public String getName(){return name;}
}
//// core/game/world/GameWorld.java
package core.game.world;
import oracle.Rec;
public class GameWorld {
 public static final class Settings{public String getName(){return "2009scape";}}
 public static Settings getSettings(){Rec.emit("settings");return new Settings();}
}
//// core/game/system/config/ItemConfigParser.java
package core.game.system.config;
public class ItemConfigParser { public static final String DESTROY_MESSAGE="destroy_message"; }
//// core/net/packet/PacketRepository.java
package core.net.packet;
import oracle.Rec;import core.net.packet.context.ChildPositionContext;
public class PacketRepository {
 public static void send(Class<?> type,Object context){ChildPositionContext c=(ChildPositionContext)context;Rec.emit("childPosition",type.getSimpleName(),c.player==null?null:c.player.oid,c.iface,c.child,c.x,c.y);}
}
//// core/net/packet/context/ChildPositionContext.java
package core.net.packet.context;
import core.game.node.entity.player.Player;
public class ChildPositionContext { public final Player player;public final int iface,child,x,y;public ChildPositionContext(Player p,int iface,int child,int x,int y){player=p;this.iface=iface;this.child=child;this.x=x;this.y=y;} }
//// core/net/packet/context/ContainerContext.java
package core.net.packet.context;
public class ContainerContext { }
//// core/net/packet/out/ContainerPacket.java
package core.net.packet.out;
public class ContainerPacket { }
//// core/net/packet/out/RepositionChild.java
package core.net.packet.out;
public class RepositionChild { }
//// core/tools/Log.java
package core.tools;
public enum Log { FINE, INFO, WARN, ERR, DEBUG }
//// core/tools/SystemLogger.java
package core.tools;
public final class SystemLogger { }
//// core/tools/StringUtils.java
package core.tools;
public final class StringUtils {
 /* ORIGINAL_FORMAT_DISPLAY_NAME */
}
//// core/game/interaction/OptionHandler.java
package core.game.interaction;
import core.game.node.Node;import core.game.node.entity.player.Player;
public abstract class OptionHandler implements core.plugin.Plugin<Object> {
 public abstract boolean handle(Player player,Node node,String option);
 public boolean isWalk(){return true;}
 public Object fireEvent(String identifier,Object...args){return null;}
}
//// content/global/skill/slayer/SlayerManager.java
package content.global.skill.slayer;
import oracle.Rec;import core.game.node.entity.player.Player;
public class SlayerManager {
 final Player player;SlayerManager(Player p){player=p;}
 public static SlayerManager getInstance(Player p){return new SlayerManager(p);}
 public boolean hasStarted(){Rec.emit("slayerStarted",player.oid);return oracle.Oracle.flag("slayer");}
}
