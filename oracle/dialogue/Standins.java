// Recording stand-ins for the systems the unmodified dialogue classes reach.
// generate.py splits this bundle at each "//// path" marker into one Java file
// per public class. None of these is a port of the class it names: every method
// records its call through oracle.Rec and returns scenario-controlled state.
//// oracle/Rec.java
package oracle;
import java.util.*;
public final class Rec {
 public static Map input=new HashMap();public static List<Object> events=new ArrayList<>();public static Map<String,Integer> calls=new HashMap<>();public static Set<Integer> fired=new HashSet<>();
 public interface Driver{void hook(Map action);}
 public static Driver driver;
 public static final class Failure extends RuntimeException{}
 public static int n(Object o){return((Number)o).intValue();}
 /** Text as a JSON string, or as its UTF-16 code units when it holds a surrogate. */
 public static Object units(String s){if(s==null)return null;boolean plain=true;for(char c:s.toCharArray())plain&=!Character.isSurrogate(c);if(plain)return s;List<Integer> out=new ArrayList<>();for(char c:s.toCharArray())out.add((int)c);return out;}
 public static void reset(Map scenario){input=scenario;events=new ArrayList<>();calls=new HashMap<>();fired=new HashSet<>();}
 public static void emit(String phase,Object...args){
  List<Object> row=new ArrayList<>();row.add(phase);row.addAll(Arrays.asList(args));events.add(row);
  int at=calls.merge(phase,1,Integer::sum);
  List hs=(List)input.getOrDefault("hooks",Collections.emptyList());
  for(int i=0;i<hs.size();i++){Map h=(Map)hs.get(i);int want=h.get("at")==null?1:n(h.get("at"));if(phase.equals(h.get("event"))&&at==want&&fired.add(i))for(Object a:(List)h.get("actions"))driver.hook((Map)a);}
  Map fails=(Map)input.getOrDefault("failures",Collections.emptyMap());
  if(fails.containsKey(phase)&&n(fails.get(phase))==at)throw new Failure();
 }
}
//// core/game/node/entity/Entity.java
package core.game.node.entity;
public abstract class Entity { public abstract int getId(); }
//// core/game/node/entity/npc/NPC.java
package core.game.node.entity.npc;
import oracle.Rec;import core.game.node.entity.player.Player;
public class NPC extends core.game.node.entity.Entity {
 public static final class Queue{final int oid;Queue(int o){oid=o;}public void reset(){Rec.emit("walkReset",oid);}}
 public static final class Pulses{final int oid;Pulses(int o){oid=o;}public void clear(){Rec.emit("pulseClear",oid);}}
 public final int oid;public int rawId;public NPC rawShown;public boolean noQueue,noPulses;public Player rawDialoguePlayer;
 public NPC(int oid,int id){this.oid=oid;this.rawId=id;this.rawShown=this;}
 public int getId(){Rec.emit("npcId",oid);return rawId;}
 public void setDialoguePlayer(Player p){Rec.emit("dialoguePlayer",oid,p==null?null:p.oid);rawDialoguePlayer=p;}
 public Queue getWalkingQueue(){Rec.emit("walkingQueue",oid);return noQueue?null:new Queue(oid);}
 public Pulses getPulseManager(){Rec.emit("pulseManager",oid);return noPulses?null:new Pulses(oid);}
 public NPC getShownNPC(Player p){Rec.emit("shownNpc",oid,p==null?null:p.oid);return rawShown;}
}
//// core/game/node/entity/player/Player.java
package core.game.node.entity.player;
import oracle.Rec;import java.util.*;
import core.game.node.entity.player.link.*;import core.game.dialogue.DialogueInterpreter;import core.game.event.Event;
public class Player extends core.game.node.entity.Entity {
 public final int oid;private PacketDispatch dispatch;private InterfaceManager manager;private DialogueInterpreter interpreter;
 private String username;private boolean male=true,debug;private int nodeId;private final Map<String,Object> attributes=new TreeMap<>();
 public Player(int oid){this.oid=oid;this.username="Player"+oid;}
 public void raw(PacketDispatch d,InterfaceManager m,DialogueInterpreter i){dispatch=d;manager=m;interpreter=i;}
 public PacketDispatch rawDispatch(){return dispatch;}public InterfaceManager rawManager(){return manager;}public DialogueInterpreter rawInterpreter(){return interpreter;}
 public void rawDispatch(PacketDispatch d){dispatch=d;}public void rawManager(InterfaceManager m){manager=m;}public void rawInterpreter(DialogueInterpreter i){interpreter=i;}
 public void rawUsername(String s){username=s;}public void rawMale(boolean b){male=b;}public void rawDebug(boolean b){debug=b;}public void rawNodeId(int i){nodeId=i;}
 public Map<String,Object> rawAttributes(){return attributes;}
 public int getId(){Rec.emit("playerId",oid);return nodeId;}
 public PacketDispatch getPacketDispatch(){Rec.emit("dispatch",oid);return dispatch;}
 public InterfaceManager getInterfaceManager(){Rec.emit("manager",oid);return manager;}
 public DialogueInterpreter getDialogueInterpreter(){Rec.emit("interpreter",oid);return interpreter;}
 public String getUsername(){Rec.emit("username",oid);return username;}
 public boolean isMale(){Rec.emit("male",oid);return male;}
 public boolean isDebug(){Rec.emit("isDebug",oid);return debug;}
 public void sendMessage(String s){Rec.emit("message",oid,Rec.units(s));}
 public void debug(String s){Rec.emit("debug",oid,Rec.units(s));}
 public void setAttribute(String key,Object value){Rec.emit("setAttribute",oid,key,oracle.Describe.value(value));attributes.put(key,value);}
 @SuppressWarnings("unchecked") public <T> T getAttribute(String key,T fail){Rec.emit("getAttribute",oid,key);Object v=attributes.get(key);return v==null?fail:(T)v;}
 public void dispatch(Event e){Rec.emit("event",oid,oracle.Describe.event(e));}
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
}
//// core/game/node/entity/player/link/InterfaceManager.java
package core.game.node.entity.player.link;
import oracle.Rec;import core.game.component.Component;
public class InterfaceManager {
 public final int oid;private Component chatbox;public final Component base;public boolean veto;
 public InterfaceManager(int oid){this.oid=oid;this.base=new Component(137);this.chatbox=base;}
 public Component rawChatbox(){return chatbox;}public void rawChatbox(Component c){chatbox=c;}
 public Component openChatbox(int iface){chatbox=new Component(iface);Rec.emit("open",oid,iface,chatbox.ref);return chatbox;}
 public boolean closeChatbox(){Rec.emit("closeChatbox",oid);if(!veto)chatbox=base;return !veto;}
 public Component getChatbox(){Rec.emit("chatbox",oid);return chatbox;}
}
//// core/game/component/Component.java
package core.game.component;
import oracle.Rec;import java.util.*;
public class Component {
 public static List<Component> all=new ArrayList<>();
 public final int ref;private int id;private Object closeEvent;
 public Component(int id){this.id=id;this.ref=all.size();all.add(this);}
 public int rawId(){return id;}public void rawId(int i){id=i;}public void rawCloseEvent(Object o){closeEvent=o;}public Object rawCloseEvent(){return closeEvent;}
 public int getId(){Rec.emit("id",ref);return id;}
 public Object getCloseEvent(){Rec.emit("closeEvent",ref);return closeEvent;}
}
//// core/game/node/item/Item.java
package core.game.node.item;
import oracle.Rec;
public class Item {
 public static java.util.List<Item> all=new java.util.ArrayList<>();
 public final int ref;public int rawId,rawAmount;public String rawName;
 public Item(int id){this(id,1);}
 public Item(int id,int amount){rawId=id;rawAmount=amount;rawName="Item "+id;ref=all.size();all.add(this);Rec.emit("newItem",ref,id,amount);}
 public int getId(){return rawId;}public int getAmount(){return rawAmount;}public String getName(){return rawName;}
}
//// core/cache/def/impl/ItemDefinition.java
package core.cache.def.impl;
import oracle.Rec;import java.util.*;
public class ItemDefinition {
 public static Map<Integer,ItemDefinition> all=new TreeMap<>();
 public final int id;public int zoom,rotationX,rotationY;public String name;public Map<String,Object> handlers=new HashMap<>();
 public ItemDefinition(int id){this.id=id;this.zoom=2000+id;this.rotationX=id*3;this.rotationY=id*7;this.name="Def "+id;}
 public static ItemDefinition forId(int id){Rec.emit("itemDef",id);return all.computeIfAbsent(id,ItemDefinition::new);}
 public int getModelZoom(){return zoom;}public int getModelRotationX(){return rotationX;}public int getModelRotationY(){return rotationY;}public String getName(){return name;}
 @SuppressWarnings("unchecked") public <T> T getConfiguration(String key,T fail){Object v=handlers.get(key);return v==null?fail:(T)v;}
}
//// core/cache/def/impl/NPCDefinition.java
package core.cache.def.impl;
import oracle.Rec;import java.util.*;
public class NPCDefinition {
 public static Map<Integer,String> names=new TreeMap<>();public static Set<Integer> missing=new TreeSet<>();
 private final String name;private NPCDefinition(String n){name=n;}
 public static NPCDefinition forId(int id){Rec.emit("npcDef",id);if(missing.contains(id))return null;return new NPCDefinition(names.containsKey(id)?names.get(id):"Npc "+id);}
 public String getName(){return name;}
}
//// core/game/world/GameWorld.java
package core.game.world;
import oracle.Rec;
public class GameWorld {
 public static final class Settings{public String name="2009scape";public String getName(){return name;}}
 public static Settings settings=new Settings();public static boolean noSettings;
 public static Settings getSettings(){Rec.emit("settings");return noSettings?null:settings;}
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
//// core/api/ContentAPIKt.java
package core.api;
import oracle.Rec;import core.tools.Log;
public final class ContentAPIKt {
 public static void log(Class<?> origin,Log type,String message){Rec.emit("log",origin.getSimpleName(),type.name(),Rec.units(message));}
}
