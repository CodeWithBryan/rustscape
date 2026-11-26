"""Recording adapters for the activities oracle.

The dynamic-regions adapters (oracle/dynamic-regions/adapters.py) are the base:
DynamicRegion, Region, RegionChunk and RegionManager run unchanged over the
real cache regions here too. This file replaces the entity, zone, world and
ContentAPI stand-ins with ones that also answer the activity and cutscene
framework, and adds the scripted oracle content (TestActivity,
TestCutscenePlugin, TestCutscene) that calls only inherited original members.

`/* ORIGINAL_* */` markers are replaced with pinned original text at
generation time (see generate.py `markers`). Adapter fields never share a
name with a getter's Kotlin property.
"""
import importlib.util
import pathlib

HERE = pathlib.Path(__file__).resolve().parent


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = _load('activities_base_adapters', HERE.parent / 'dynamic-regions' / 'adapters.py').ADAPTERS

REPLACED = [
    'core/ServerConstants.java',
    'core/api/ContentAPIKt.java',
    'core/api/Logger.kt',
    'core/game/world/GameWorld.java',
    'core/game/node/entity/Entity.java',
    'core/game/node/entity/player/Player.java',
    'core/game/node/entity/npc/NPC.java',
    'content/global/skill/summoning/familiar/Familiar.java',
    'core/game/world/map/zone/MapZone.java',
    'core/game/world/map/zone/RegionZone.java',
    'core/game/world/map/zone/impl/MultiwayCombatZone.java',
    'core/net/IoSession.java',
    'core/game/world/map/Location.kt',
    'core/game/node/item/Item.java',
    'core/game/world/map/zone/ZoneMonitor.java',
]

OWN = {
    'core/ServerConstants.kt': '''package core
import core.game.world.map.Location
class ServerConstants {
    companion object {
        @JvmField var HOME_LOCATION: Location? = null
        @JvmField var LOG_CUTSCENE = true
        @JvmField var SERVER_NAME: String = ""
    }
}
''',
    'core/game/world/GameWorld.kt': '''package core.game.world
import core.game.system.task.Pulse
object GameWorld {
    @JvmStatic var Pulser = PulseRunner()
    @JvmStatic var settings: GameSettings? = null
    @JvmStatic fun prompt(b: Boolean) { throw UnsupportedOperationException() }
}
class PulseRunner {
    fun submit(pulse: Pulse) = oracle.Support.submit(pulse)
}
class GameSettings(var name: String, var isDevMode: Boolean)
''',
    'core/api/ContentAPI.kt': '''package core.api
import core.game.activity.Cutscene
import core.game.dialogue.FacialExpression
import core.game.interaction.QueueStrength
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.component.Component
import core.tools.Log
fun log(origin: Class<*>, type: Log, message: String) = oracle.Support.emit("log", type.name, message)
fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean) {
    oracle.Support.emit("queueScript", entity.name, delay, strength.name, persist)
}
fun restartScript(entity: Entity): Boolean = throw UnsupportedOperationException()
fun setVarbit(player: Player, varbit: Int, value: Int): Unit = throw UnsupportedOperationException()
fun setVarp(player: Player, varp: Int, value: Int): Unit = throw UnsupportedOperationException()
fun getVarp(player: Player, varp: Int): Int = throw UnsupportedOperationException()
fun stopExecuting(entity: Entity): Boolean = throw UnsupportedOperationException()
/* ORIGINAL_GET_ATTRIBUTE */
/* ORIGINAL_SET_ATTRIBUTE */
/* ORIGINAL_REMOVE_ATTRIBUTE */
/* ORIGINAL_OPEN_INTERFACE */
/* ORIGINAL_CLOSE_INTERFACE */
/* ORIGINAL_GET_CUTSCENE */
/* ORIGINAL_GET_CUTSCENE_STAGE */
fun sendNPCDialogue(player: Player, npc: Int, msg: String, expr: FacialExpression = FacialExpression.NEUTRAL, hide: Boolean = false) =
    oracle.Support.emit("sendNPCDialogue", player.name, npc, msg, expr.name, hide)
fun sendNPCDialogueLines(player: Player, npc: Int, expr: FacialExpression, hideContinue: Boolean, vararg msgs: String) =
    oracle.Support.emit("sendNPCDialogueLines", player.name, npc, expr.name, hideContinue, msgs.toList())
fun closeDialogue(player: Player) = oracle.Support.emit("closeDialogue", player.name)
fun sendDialogue(player: Player, message: String) = oracle.Support.emit("sendDialogue", player.name, message)
fun sendDialogueLines(player: Player, vararg message: String) = oracle.Support.emit("sendDialogueLines", player.name, message.toList())
fun sendPlayerDialogue(player: Player, msg: String, expr: FacialExpression = FacialExpression.NEUTRAL, hide: Boolean = false) =
    oracle.Support.emit("sendPlayerDialogue", player.name, msg, expr.name, hide)
fun storeBookInHouse(player: Player, node: Node): Boolean {
    oracle.Support.emit("storeBookInHouse", player.name, oracle.Support.key(node))
    return oracle.Support.answer("storeBookInHouse", true)
}
''',
    'core/game/system/timer/impl/AntiMacro.kt': '''package core.game.system.timer.impl
import core.game.node.entity.player.Player
class AntiMacro {
    companion object {
        fun pause(player: Player) = oracle.Support.emit("antiMacroPause", player.name)
        fun unpause(player: Player) = oracle.Support.emit("antiMacroUnpause", player.name)
    }
}
''',
    'core/api/utils/PlayerCamera.kt': '''package core.api.utils
import core.game.node.entity.player.Player
class PlayerCamera(val player: Player?) {
    init { oracle.Support.emit("camera", player?.name) }
    fun rotateTo(x: Int, y: Int, height: Int, speed: Int) = oracle.Support.emit("cameraRotateTo", player?.name, x, y, height, speed)
    fun rotateBy(diffX: Int, diffY: Int, diffHeight: Int, speed: Int) = oracle.Support.emit("cameraRotateBy", player?.name, diffX, diffY, diffHeight, speed)
    fun panTo(x: Int, y: Int, height: Int, speed: Int) = oracle.Support.emit("cameraPanTo", player?.name, x, y, height, speed)
    fun shake(cameraType: Int, jitter: Int, amplitude: Int, frequency: Int, speed: Int) = oracle.Support.emit("cameraShake", player?.name, cameraType, jitter, amplitude, frequency, speed)
    fun reset() = oracle.Support.emit("cameraReset", player?.name)
}
''',
    'core/api/Event.kt': '''package core.api
import core.game.event.*
object Event {
    @JvmStatic val SelfDeath = SelfDeathEvent::class.java
}
''',
    'core/game/event/Events.kt': '''package core.game.event
import core.game.node.entity.Entity
data class SelfDeathEvent(val killer: Entity) : Event
''',
    'core/game/node/item/Item.java': 'package core.game.node.item;public class Item extends core.game.node.Node{public final int itemId;public Item(int id){super(null,null);itemId=id;}public Item(int id,int amount){this(id);}public int getId(){return itemId;}}',
    'core/game/world/map/zone/ZoneMonitor.java': '''package core.game.world.map.zone;
public class ZoneMonitor{
 private final core.game.node.entity.Entity owner;
 public ZoneMonitor(core.game.node.entity.Entity e){owner=e;}
 public boolean updateLocation(core.game.world.map.Location l){oracle.Support.emit("zoneUpdate",owner.getName(),oracle.Support.coords(l));return true;}
 public boolean isIgnoreMultiBoundaries(core.game.node.entity.Entity t){throw new UnsupportedOperationException();}
}''',
    'core/game/interaction/Option.java': 'package core.game.interaction;public class Option{}',
    'core/game/interaction/MovementPulse.java': '''package core.game.interaction;
import core.game.node.Node;import core.game.node.entity.Entity;import core.game.world.map.Location;import core.game.world.map.path.Pathfinder;
public abstract class MovementPulse extends core.game.system.task.Pulse{
 public final Entity mover;public final Location target;
 public MovementPulse(Entity mover,Object destination,Pathfinder finder){super(1,mover);this.mover=mover;this.target=destination instanceof Location?(Location)destination:null;}
}''',
    'core/game/world/map/path/Pathfinder.java': 'package core.game.world.map.path;public class Pathfinder{public static final Pathfinder SMART=new Pathfinder();}',
    'core/game/node/entity/combat/CombatStyle.java': 'package core.game.node.entity.combat;public enum CombatStyle{MELEE,RANGE,MAGIC}',
    'core/game/node/entity/player/link/request/RequestType.java': 'package core.game.node.entity.player.link.request;public class RequestType{}',
    'core/game/component/Component.java': '''package core.game.component;
public class Component{private final int componentId;public Component(int id){componentId=id;}public int getId(){return componentId;}}''',
    'core/game/node/entity/Entity.java': '''package core.game.node.entity;
import core.game.node.Node;import core.game.world.map.Location;import core.game.world.map.zone.ZoneMonitor;
import core.game.event.Event;import core.game.event.EventHook;import core.game.system.task.Pulse;
import java.util.*;
public class Entity extends Node{
 private final ZoneMonitor monitor=new ZoneMonitor(this);private final Walking walking=new Walking(this);
 private final Properties propertiesObject=new Properties(this);private final Locks locksObject=new Locks(this);
 private final PulseManager pulses=new PulseManager(this);
 public final Map<String,Object> attributeMap=new HashMap<>();
 public final List<Object> hookList=new ArrayList<>();
 public Entity(String name,Location l){super(name,l);}
 public ZoneMonitor getZoneMonitor(){return monitor;}
 public Walking getWalkingQueue(){return walking;}
 public Properties getProperties(){return propertiesObject;}
 public Locks getLocks(){return locksObject;}
 public PulseManager getPulseManager(){return pulses;}
 public String getUsername(){return name;}
 public core.game.node.entity.player.Player asPlayer(){return(core.game.node.entity.player.Player)this;}
 public void setLocation(Location l){oracle.Support.emit("setLocation",name,oracle.Support.coords(l));location=l;}
 public void setAttribute(String key,Object value){oracle.Support.emit("setAttribute",name,key,oracle.Support.value(value));attributeMap.put(key,value);}
 public <T> T getAttribute(String key,T fail){oracle.Support.emit("getAttribute",name,key);Object o=attributeMap.get(key);if(o==null)return fail;return(T)o;}
 public void removeAttribute(String key){oracle.Support.emit("removeAttribute",name,key);attributeMap.remove(key);}
 public void lock(){oracle.Support.emit("lock",name);}
 public void unlock(){oracle.Support.emit("unlock",name);}
 public void faceLocation(Location l){oracle.Support.emit("faceLocation",name,oracle.Support.coords(l));}
 public boolean isIgnoreMultiBoundaries(Entity t){throw new UnsupportedOperationException();}
 public int getId(){throw new UnsupportedOperationException();}
 public <T extends Event> void hook(Class<T> event,EventHook<T> hook){oracle.Support.emit("hook",name,event.getSimpleName());hookList.add(hook);}
 public <T extends Event> void unhook(EventHook<T> hook){oracle.Support.emit("unhook",name);hookList.remove(hook);}
 public static class Walking{
  private final Entity owner;Walking(Entity e){owner=e;}
  public Location getFootPrint(){return owner.getLocation();}
  public void update(){oracle.Support.emit("walkingUpdate",owner.getName());}
  public void reset(){oracle.Support.emit("walkingReset",owner.getName());}
 }
 public static class Properties{
  private final Entity owner;Properties(Entity e){owner=e;}
  public Location teleportTarget,spawnTarget;public boolean safe;
  public Location safeRespawn=null;
  public void setTeleportLocation(Location l){oracle.Support.emit("teleportLocation",owner.getName(),oracle.Support.coords(l));teleportTarget=l;}
  public void setSpawnLocation(Location l){oracle.Support.emit("spawnLocation",owner.getName(),oracle.Support.coords(l));spawnTarget=l;}
  public void setSafeZone(boolean v){oracle.Support.emit("safeZone",owner.getName(),v);safe=v;}
  public boolean isSafeZone(){return safe;}
  public Location getTeleportLocation(){return teleportTarget;}
  public boolean isMultiZone(){throw new UnsupportedOperationException();}
 }
 public static class Locks{
  private final Entity owner;Locks(Entity e){owner=e;}
  public void lockMovement(int ticks){oracle.Support.emit("lockMovement",owner.getName(),ticks);}
  public void unlockMovement(){oracle.Support.emit("unlockMovement",owner.getName());}
 }
 public static class PulseManager{
  private final Entity owner;PulseManager(Entity e){owner=e;}
  public void run(Pulse p){core.game.interaction.MovementPulse m=(core.game.interaction.MovementPulse)p;oracle.Support.emit("movementPulse",owner.getName(),oracle.Support.coords(m.target));}
 }
}''',
    'core/game/node/entity/player/Player.java': '''package core.game.node.entity.player;
import core.game.node.entity.Entity;import core.game.world.map.*;import core.game.world.map.build.DynamicRegion;
import core.game.component.Component;import kotlin.jvm.functions.Function1;import kotlin.jvm.functions.Function2;
import core.net.packet.PacketRepository;import core.net.packet.context.*;import core.net.packet.out.*;
import java.util.*;
public class Player extends Entity{
 public Location lastScene;private final Flags flagsObject=new Flags(this);private final Details detailsObject=new Details();
 private final Interfaces interfacesObject=new Interfaces(this);private final Dispatch dispatchObject=new Dispatch(this);
 private final Interpreter interpreterObject=new Interpreter(this);
 public final Listeners listenerMap=new Listeners(this);
 public Player(String name,Location l){super(name,l);}
 public core.net.IoSession getSession(){return oracle.Support.session;}
 public Details getDetails(){return detailsObject;}
 public Flags getPlayerFlags(){return flagsObject;}
 public Interfaces getInterfaceManager(){return interfacesObject;}
 public Dispatch getPacketDispatch(){return dispatchObject;}
 public Interpreter getDialogueInterpreter(){return interpreterObject;}
 public HashMap<String,Function1<Player,kotlin.Unit>> getLogoutListeners(){return listenerMap;}
 public boolean isArtificial(){return false;}
 public Holder getInventory(){throw new UnsupportedOperationException();}
 public Holder getEquipment(){throw new UnsupportedOperationException();}
 public Holder getBank(){throw new UnsupportedOperationException();}
 public static class Holder{public boolean containsItem(core.game.node.item.Item i){throw new UnsupportedOperationException();}public int getAmount(core.game.node.item.Item i){throw new UnsupportedOperationException();}public boolean remove(core.game.node.item.Item i){throw new UnsupportedOperationException();}}
 public boolean isPlaying(){throw new UnsupportedOperationException("private ground items are outside this corpus");}
 public static class Details{public core.net.IoSession getSession(){return oracle.Support.session;}}
 public static class Flags{private final Player owner;Flags(Player p){owner=p;}public void setLastSceneGraph(Location l){owner.lastScene=l;}public Location getLastSceneGraph(){return owner.lastScene;}}
 public static class Interfaces{
  private final Player owner;Interfaces(Player p){owner=p;}
  public void removeTabs(int... tabs){List<Integer> l=new ArrayList<>();for(int t:tabs)l.add(t);oracle.Support.emit("removeTabs",owner.getName(),l);}
  public void restoreTabs(){oracle.Support.emit("restoreTabs",owner.getName());}
  public void openDefaultTabs(){oracle.Support.emit("openDefaultTabs",owner.getName());}
  public boolean close(){oracle.Support.emit("closeInterface",owner.getName());return oracle.Support.answer("closeInterface",true);}
  public void open(Component c){oracle.Support.emit("openInterface",owner.getName(),c.getId());}
  public void openOverlay(Component c){oracle.Support.emit("openOverlay",owner.getName(),c.getId());}
  public void closeOverlay(){oracle.Support.emit("closeOverlay",owner.getName());}
 }
 public static class Dispatch{
  private final Player owner;Dispatch(Player p){owner=p;}
  public void sendMessage(String m){oracle.Support.emit("packetMessage",owner.getName(),m);}
  public void sendString(String s,int iface,int child){oracle.Support.emit("sendString",owner.getName(),s,iface,child);}
  public void sendInterfaceConfig(int iface,int child,boolean hidden){oracle.Support.emit("interfaceConfig",owner.getName(),iface,child,hidden);}
  public void sendModelOnInterface(int model,int iface,int child,int zoom){oracle.Support.emit("modelOnInterface",owner.getName(),model,iface,child,zoom);}
  public void sendAngleOnInterface(int iface,int child,int zoom,int pitch,int yaw){oracle.Support.emit("angleOnInterface",owner.getName(),iface,child,zoom,pitch,yaw);}
  public void sendItemOnInterface(int item,int amount,int iface,int child){oracle.Support.emit("itemOnInterface",owner.getName(),item,amount,iface,child);}
 }
 public static class Interpreter{
  private final Player owner;Interpreter(Player p){owner=p;}
  public void addAction(Function2<Player,Integer,kotlin.Unit> action){oracle.Support.emit("addAction",owner.getName());oracle.Support.actions.add(action);}
 }
 public static class Listeners extends HashMap<String,Function1<Player,kotlin.Unit>>{
  private final Player owner;Listeners(Player p){owner=p;}
  @Override public Function1<Player,kotlin.Unit> put(String k,Function1<Player,kotlin.Unit> v){oracle.Support.emit("logoutListener",owner.getName(),k);return super.put(k,v);}
  @Override public Function1<Player,kotlin.Unit> remove(Object k){oracle.Support.emit("removeLogoutListener",owner.getName(),k);return super.remove(k);}
 }
 /* ORIGINAL_UPDATE_SCENE_GRAPH */
}''',
    'core/game/node/entity/npc/NPC.java': '''package core.game.node.entity.npc;
import core.game.node.entity.Entity;import core.game.world.map.*;
public class NPC extends Entity{
 private final int npcId;private boolean aggressiveFlag,respawnFlag,walksFlag;
 public NPC(String name,int id,Location l,Direction d){super(name,l);npcId=id;direction=d;}
 public NPC(int id){super(oracle.Support.npcName(),null);npcId=id;oracle.Support.emit("npcNew",name,id);}
 public static NPC create(int id,Location l,Direction d){NPC n=new NPC(oracle.Support.copyName(),id,l,d);oracle.Support.emit("npcCreate",n.getName(),id,oracle.Support.coords(l),d==null?null:d.name());return n;}
 public int getId(){return npcId;}
 public boolean isAggressive(){return aggressiveFlag;}
 public boolean isRespawn(){return respawnFlag;}
 public boolean isWalks(){return walksFlag;}
 public void setAggressive(boolean v){oracle.Support.emit("npcAggressive",name,v);aggressiveFlag=v;}
 public void setRespawn(boolean v){oracle.Support.emit("npcRespawn",name,v);respawnFlag=v;}
 public void setWalks(boolean v){oracle.Support.emit("npcWalks",name,v);walksFlag=v;}
 public void init(){oracle.Support.emit("npcInit",name);}
 public void clear(){oracle.Support.emit("npcClear",name);}
 public void setInvisible(boolean v){oracle.Support.emit("setInvisible",name,v);}
 public void onRegionInactivity(){oracle.Support.emit("regionInactivity",name);}
}''',
    'content/global/skill/summoning/familiar/Familiar.java': '''package content.global.skill.summoning.familiar;
import core.game.node.entity.player.Player;import core.game.world.map.*;
public class Familiar extends core.game.node.entity.npc.NPC{
 public Familiar(String name,int id,Location l){super(name,id,l,null);}
 public Player getOwner(){throw new UnsupportedOperationException("familiars are outside this corpus");}
 public void setInvisible(boolean v){oracle.Support.emit("setInvisible",name,v);}
 public boolean call(){throw new UnsupportedOperationException("the recall script never runs in this corpus");}
}''',
    'core/game/world/map/zone/impl/MultiwayCombatZone.java': '''package core.game.world.map.zone.impl;
public final class MultiwayCombatZone extends core.game.world.map.zone.MapZone{
 // The original's name and overlappable flag; its configure (the world's multiway areas) is never called here.
 private static final MultiwayCombatZone INSTANCE=new MultiwayCombatZone();
 private MultiwayCombatZone(){super("Multicombat",true);}
 public static MultiwayCombatZone getInstance(){return INSTANCE;}
}''',
    'core/net/IoSession.java': '''package core.net;
import java.nio.ByteBuffer;import core.net.packet.IoBuffer;import core.net.event.GameWriteEvent;
public class IoSession{
 public boolean isActive(){oracle.Support.emit("sessionActive");return oracle.Support.answer("sessionActive",true);}
 public void disconnect(){throw new UnsupportedOperationException("outside scope");}
 public void queue(ByteBuffer b){byte[] bytes=new byte[b.remaining()];b.get(bytes);oracle.Support.frame(bytes);}
 public void write(IoBuffer b){oracle.Support.emit("write");new GameWriteEvent(this,b).write(this,b);}
 public IoSession getIsaacPair(){return this;}
 public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}
}''',
    'core/game/world/map/Location.kt': '''package core.game.world.map
import kotlin.math.*
class Location(var hash: LocationHash) {
    @JvmOverloads constructor(x: Int, y: Int, z: Int = 0) : this(LocationHash(x, y, (z % 4 + 4) % 4))
    val x: Int get() = hash.x
    val y: Int get() = hash.y
    val z: Int get() = hash.z
    companion object {
        @JvmStatic @JvmOverloads fun create(x: Int, y: Int, z: Int = 0): Location = Location(x, y, z)
        /* ORIGINAL_GET_DISTANCE */
    }
    /* ORIGINAL_LOCATION_MEMBERS */
    /* ORIGINAL_LOCATION_TRANSFORM_DIRECTION */
    @JvmInline
    /* ORIGINAL_LOCATION_HASH */
}
''',
    # ---------------------------------------------------------------- scripted oracle content
    'oracle/content/TestActivity.java': '''package oracle.content;
import core.game.activity.ActivityPlugin;import core.game.node.entity.player.Player;import core.game.world.map.Location;
import core.game.world.map.build.DynamicRegion;import core.game.world.map.zone.*;import java.util.*;
/** Scripted content: an ActivityPlugin subclass that calls only inherited original members. */
public class TestActivity extends ActivityPlugin{
 public final Map config;
 public TestActivity(Map config){
  super((String)config.get("name"),oracle.Support.b(config,"instanced"),oracle.Support.b(config,"multicombat"),oracle.Support.b(config,"safe"),oracle.Support.restrictions(config));
  this.config=config;oracle.Support.track(this,"activity");
 }
 @Override public ActivityPlugin newInstance(Player p)throws Throwable{oracle.Support.emit("newInstance",oracle.Support.label(this),p==null?null:p.getName());return new TestActivity(config);}
 @Override public Location getSpawnLocation(){return oracle.Support.location(config.get("spawn"));}
 @Override public void register(){oracle.Support.emit("contentRegister",oracle.Support.label(this));}
 @Override public void configure(){oracle.Support.emit("contentConfigure",oracle.Support.label(this));oracle.Support.configure(this,config,new Object[]{this});}
 @Override public boolean start(Player player,boolean login,Object... args){oracle.Support.emit("contentStart",oracle.Support.label(this),login,Arrays.asList(args));return super.start(player,login,args);}
 public void setRegion(DynamicRegion r){region=r;}
 public void callSetRegionBase(){setRegionBase();}
 public void callSetRegionBase(DynamicRegion[] r){setRegionBase(r);}
}''',
    'oracle/content/TestCutscenePlugin.java': '''package oracle.content;
import core.game.activity.ActivityPlugin;import core.game.activity.CutscenePlugin;import core.game.node.entity.player.Player;import core.game.world.map.Location;
import core.game.world.map.build.DynamicRegion;import java.util.*;
/** Scripted content: a CutscenePlugin subclass that calls only inherited original members. */
public class TestCutscenePlugin extends CutscenePlugin{
 public final Map config;
 public TestCutscenePlugin(Map config){
  super((String)config.get("name"),!config.containsKey("fade")||oracle.Support.b(config,"fade"));
  this.config=config;oracle.Support.track(this,"cutscenePlugin");
 }
 @Override public ActivityPlugin newInstance(Player p)throws Throwable{oracle.Support.emit("newInstance",oracle.Support.label(this),p==null?null:p.getName());return new TestCutscenePlugin(config);}
 @Override public Location getSpawnLocation(){return oracle.Support.location(config.get("spawn"));}
 @Override public void configure(){oracle.Support.emit("contentConfigure",oracle.Support.label(this));oracle.Support.configure(this,config,new Object[]{this});}
 @Override public void open(){oracle.Support.emit("open",oracle.Support.label(this));}
 @Override public void fade(){oracle.Support.emit("fade",oracle.Support.label(this));}
 @Override public int getMapState(){return config.containsKey("mapState")?oracle.Support.n(config.get("mapState")):super.getMapState();}
 @Override public int[] getRemovedTabs(){if(!config.containsKey("tabs"))return super.getRemovedTabs();List l=(List)config.get("tabs");int[] a=new int[l.size()];for(int i=0;i<a.length;i++)a[i]=oracle.Support.n(l.get(i));return a;}
 @Override public Location getStartLocation(){return config.containsKey("startLocation")?oracle.Support.location(config.get("startLocation")):super.getStartLocation();}
 public void setRegion(DynamicRegion r){region=r;}
 public void callSetRegionBase(){setRegionBase();}
 public void callSetRegionBase(DynamicRegion[] r){setRegionBase(r);}
}''',
    'oracle/content/TestCutscene.kt': '''package oracle.content
import core.api.utils.CameraShakeType
import core.game.activity.Cutscene
import core.game.dialogue.FacialExpression
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.map.Direction
import oracle.Support
/** Scripted content: a Cutscene whose setup and stages run the case's actions through Cutscene's own members. */
class TestCutscene(player: Player, val script: Map<*, *>) : Cutscene(player) {
    init { Support.track(this, "cutscene") }
    override fun setup() {
        Support.emit("setup", Support.label(this))
        run(script["setup"] as List<*>?)
    }
    override fun runStage(stage: Int) {
        Support.emit("runStage", Support.label(this), stage)
        run((script["stages"] as Map<*, *>?)?.get(stage.toString()) as List<*>?)
    }
    fun run(actions: List<*>?) {
        for (raw in actions ?: return) act(raw as Map<*, *>)
    }
    private fun n(m: Map<*, *>, k: String) = (m[k] as Number).toInt()
    private fun cont(m: Map<*, *>): (() -> Unit)? = if (m.containsKey("onContinue")) ({ run(m["onContinue"] as List<*>) }) else null
    private fun entity(m: Map<*, *>): Entity = if (m["npc"] != null) getNPC(n(m, "npc"))!! else player
    fun act(a: Map<*, *>) {
        val c = cont(a)
        when (a["do"]) {
            "loadRegion" -> loadRegion(n(a, "region"))
            "closeOverlay" -> closeOverlay()
            "fadeToBlack" -> fadeToBlack()
            "fadeFromBlack" -> fadeFromBlack()
            "teleport" -> if (a.containsKey("plane")) teleport(entity(a), n(a, "x"), n(a, "y"), n(a, "plane")) else teleport(entity(a), n(a, "x"), n(a, "y"))
            "move" -> move(entity(a), n(a, "x"), n(a, "y"))
            "npcDialogue" -> {
                val e = FacialExpression.valueOf(a["expression"] as String)
                if (c != null) dialogueUpdate(n(a, "id"), e, a["message"] as String, c, a["hide"] == true)
                else dialogueUpdate(n(a, "id"), e, a["message"] as String, hide = a["hide"] == true)
            }
            "npcDialogueLines" -> {
                val e = FacialExpression.valueOf(a["expression"] as String)
                val lines = (a["lines"] as List<*>).map { it as String }.toTypedArray()
                if (c != null) dialogueLinesUpdate(n(a, "id"), e, *lines, onContinue = c) else dialogueLinesUpdate(n(a, "id"), e, *lines)
            }
            "dialogueClose" -> dialogueClose()
            "dialogue" -> if (c != null) dialogueUpdate(a["message"] as String, c) else dialogueUpdate(a["message"] as String)
            "dialogueLines" -> {
                val lines = (a["lines"] as List<*>).map { it as String }.toTypedArray()
                if (c != null) dialogueLinesUpdate(*lines, onContinue = c) else dialogueLinesUpdate(*lines)
            }
            "playerDialogue" -> {
                val e = FacialExpression.valueOf(a["expression"] as String)
                if (c != null) playerDialogueUpdate(e, a["message"] as String, c) else playerDialogueUpdate(e, a["message"] as String)
            }
            "timedUpdate" -> if (a.containsKey("stage")) timedUpdate(n(a, "ticks"), n(a, "stage")) else timedUpdate(n(a, "ticks"))
            "getNPC" -> Support.emit("gotNPC", getNPC(n(a, "id"))?.name)
            "getNPCs" -> Support.emit("gotNPCs", getNPCs(n(a, "id")).map { it.name })
            "getObject" -> {
                val o = if (a.containsKey("plane")) getObject(n(a, "x"), n(a, "y"), n(a, "plane")) else getObject(n(a, "x"), n(a, "y"))
                Support.emit("gotObject", o?.id)
            }
            "addNPC" -> {
                val d = Direction.valueOf(a["direction"] as String)
                if (a.containsKey("plane")) addNPC(n(a, "id"), n(a, "x"), n(a, "y"), d, n(a, "plane")) else addNPC(n(a, "id"), n(a, "x"), n(a, "y"), d)
            }
            "endWithoutFade" -> if (c != null || a.containsKey("endActions")) endWithoutFade { run(a["endActions"] as List<*>?) } else endWithoutFade()
            "end" -> {
                val fade = a["fade"]
                if (a.containsKey("endActions")) end(fade != false) { run(a["endActions"] as List<*>?) }
                else if (fade == null) end() else end(fade as Boolean)
            }
            "moveCamera" -> if (a.containsKey("height")) moveCamera(n(a, "x"), n(a, "y"), n(a, "height"), n(a, "speed")) else moveCamera(n(a, "x"), n(a, "y"))
            "rotateCamera" -> if (a.containsKey("height")) rotateCamera(n(a, "x"), n(a, "y"), n(a, "height"), n(a, "speed")) else rotateCamera(n(a, "x"), n(a, "y"))
            "rotateCameraBy" -> if (a.containsKey("height")) rotateCameraBy(n(a, "x"), n(a, "y"), n(a, "height"), n(a, "speed")) else rotateCameraBy(n(a, "x"), n(a, "y"))
            "shakeCamera" -> {
                val t = CameraShakeType.valueOf(a["type"] as String)
                if (a.containsKey("jitter")) shakeCamera(t, n(a, "jitter"), n(a, "amplitude"), n(a, "frequency"), n(a, "speed")) else shakeCamera(t)
            }
            "resetCamera" -> resetCamera()
            "setExit" -> setExit(Support.location(a["location"])!!)
            "incrementStage" -> incrementStage()
            "updateStage" -> updateStage(n(a, "stage"))
            "clearNPCs" -> clearNPCs()
            "log" -> logCutscene(a["message"] as String)
            "emit" -> Support.emit("content", a["value"])
            "fail" -> throw Support.Failure()
            else -> throw AssertionError(a)
        }
    }
}
''',
}

ADAPTERS = {k: v for k, v in BASE.items() if k not in REPLACED}
ADAPTERS.update(OWN)
