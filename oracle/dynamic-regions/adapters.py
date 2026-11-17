"""Recording adapters for the dynamic-regions oracle.

Every class here stands in for a system outside the compiled originals.
`/* ORIGINAL_* */` markers are replaced with the pinned original text at
generation time (see generate.py `markers`). Adapter fields never share a
name with a getter's Kotlin property.
"""

ADAPTERS = {
    'core/ServerConstants.java': 'package core;public class ServerConstants{}',
    'org/jetbrains/annotations/NotNull.java': 'package org.jetbrains.annotations;public @interface NotNull{}',
    'org/jetbrains/annotations/Nullable.java': 'package org.jetbrains.annotations;public @interface Nullable{}',
    'core/tools/Log.java': 'package core.tools;public enum Log{FINE,INFO,WARN,ERR}',
    'core/tools/SystemLogger.java': 'package core.tools;public class SystemLogger{}',
    'core/tools/StringUtils.java': 'package core.tools;public class StringUtils{/* ORIGINAL_IS_PLUS_N */}',
    'core/tools/RandomFunction.java': '''package core.tools;
public class RandomFunction{
 public static int random(int max){throw new UnsupportedOperationException("random draws are outside this corpus");}
 public static int random(int a,int b){throw new UnsupportedOperationException("random draws are outside this corpus");}
 public static int normalRandDist(int max,int intensity){throw new UnsupportedOperationException("random draws are outside this corpus");}
 public static int getRandom(int max){throw new UnsupportedOperationException("random draws are outside this corpus");}
}''',
    'core/api/ContentAPIKt.java': '''package core.api;
import core.game.node.entity.Entity;import core.game.node.entity.player.Player;
import core.game.interaction.QueueStrength;import kotlin.jvm.functions.Function1;
public class ContentAPIKt{
 public static void log(Object owner,core.tools.Log level,String s){oracle.Support.emit("log",level.name(),s);}
 public static void setVarbit(Player p,int id,int value){throw new UnsupportedOperationException();}
 public static void setVarp(Player p,int id,int value){throw new UnsupportedOperationException();}
 public static int getVarp(Player p,int id){throw new UnsupportedOperationException();}
 public static void queueScript(Entity e,int delay,QueueStrength strength,boolean persist,Function1<Integer,Boolean> script){oracle.Support.emit("queueScript",e.getName(),delay,strength.name(),persist);}
 public static boolean restartScript(Entity e){throw new UnsupportedOperationException();}
 public static boolean stopExecuting(Entity e){throw new UnsupportedOperationException();}
}''',
    'core/api/Logger.kt': 'package core.api\nfun log(owner: Any, level: core.tools.Log, message: String) = oracle.Support.emit("log", level.name, message)\n',
    'core/cache/Cache.java': '''package core.cache;
public class Cache{
 public static int getObjectDefinitionsSize(){throw new UnsupportedOperationException("definitions are supplied by the case");}
 public static Index[] getIndexes(){Index[] a=new Index[17];a[5]=new Index();a[16]=new Index();return a;}
 public static class Index{
  public byte[] getFileData(int g,int f){throw new UnsupportedOperationException("definitions are supplied by the case");}
  public byte[] getFileData(int g,int f,int[] k){return oracle.Support.landBytes(g);}
  public int getArchiveId(String s){return oracle.Support.archive(s);}
  public Index getCacheFile(){return this;}
  public byte[] getContainerUnpackedData(int g){return oracle.Support.mapBytes(g);}
 }
}''',
    'core/cache/crypto/ISAACCipher.java': 'package core.cache.crypto;public class ISAACCipher{public int getNextValue(){return 0;}}',
    'core/cache/def/impl/VarbitDefinition.java': 'package core.cache.def.impl;public class VarbitDefinition{public static VarbitDefinition forObjectID(int id){throw new UnsupportedOperationException();}public int getValue(core.game.node.entity.player.Player p){throw new UnsupportedOperationException();}}',
    'core/game/interaction/OptionHandler.java': 'package core.game.interaction;public class OptionHandler{}',
    'core/game/interaction/DestinationFlag.java': 'package core.game.interaction;public class DestinationFlag{public static final DestinationFlag OBJECT=new DestinationFlag(),LOCATION=new DestinationFlag(),ENTITY=new DestinationFlag();}',
    'core/game/interaction/InteractPlugin.java': 'package core.game.interaction;public class InteractPlugin{public InteractPlugin(core.game.node.Node n){}}',
    'core/game/interaction/QueueStrength.java': 'package core.game.interaction;public enum QueueStrength{WEAK,NORMAL,STRONG,SOFT}',
    'core/game/node/entity/impl/GameAttributes.java': 'package core.game.node.entity.impl;public class GameAttributes{}',
    'core/game/world/GameWorld.java': '''package core.game.world;
public class GameWorld{
 public static Pulser getPulser(){return new Pulser();}
 public static void prompt(boolean b){throw new UnsupportedOperationException();}
 public static class Pulser{public void submit(core.game.system.task.Pulse p){oracle.Support.submit(p);}}
}''',
    'core/game/world/repository/Repository.java': 'package core.game.world.repository;public class Repository{public static void addRenderableNPC(core.game.node.entity.npc.NPC n){oracle.Support.emit("renderable",n.getName());}}',
    'core/game/system/communication/CommunicationInfo.java': 'package core.game.system.communication;public class CommunicationInfo{}',
    'core/game/node/Node.java': '''package core.game.node;
import core.game.world.map.*;
public class Node{
 protected String name;protected Location location;protected Direction direction;protected int size=1;
 protected boolean active=true,renderable=true;protected Object destinationFlag,interactPlugin;
 public Node(String n,Location l){name=n;location=l;}
 public String getName(){return name;}
 public Location getLocation(){return location;}
 public Location getCenterLocation(){return location;}
 public void setLocation(Location l){location=l;}
 public Direction getDirection(){return direction;}
 public int size(){return size;}
 public boolean isActive(){return active;}
 public void setActive(boolean a){active=a;}
 public boolean isRenderable(){return renderable;}
 public void setRenderable(boolean v){renderable=v;}
}''',
    'core/game/node/entity/Entity.java': '''package core.game.node.entity;
import core.game.node.Node;import core.game.world.map.Location;import core.game.world.map.zone.ZoneMonitor;
public class Entity extends Node{
 private final ZoneMonitor monitor=new ZoneMonitor(this);private final Walking walking=new Walking(this);
 public Entity(String name,Location l){super(name,l);}
 public ZoneMonitor getZoneMonitor(){return monitor;}
 public Walking getWalkingQueue(){return walking;}
 public core.game.node.entity.player.Player asPlayer(){return(core.game.node.entity.player.Player)this;}
 public void setLocation(Location l){oracle.Support.emit("setLocation",name,oracle.Support.coords(l));location=l;}
 public static class Walking{
  private final Entity owner;Walking(Entity e){owner=e;}
  public Location getFootPrint(){return owner.getLocation();}
  public void update(){oracle.Support.emit("walkingUpdate",owner.getName());}
 }
}''',
    'core/game/world/map/zone/ZoneMonitor.java': '''package core.game.world.map.zone;
public class ZoneMonitor{
 private final core.game.node.entity.Entity owner;
 public ZoneMonitor(core.game.node.entity.Entity e){owner=e;}
 public boolean updateLocation(core.game.world.map.Location l){oracle.Support.emit("zoneUpdate",owner.getName(),oracle.Support.coords(l));return true;}
}''',
    'core/game/world/map/zone/MapZone.java': 'package core.game.world.map.zone;public class MapZone{private final String zoneName;public MapZone(String n){zoneName=n;}public String getName(){return zoneName;}}',
    'core/game/world/map/zone/RegionZone.java': 'package core.game.world.map.zone;public class RegionZone{private final MapZone zoneObject;private final ZoneBorders area;public RegionZone(MapZone z,ZoneBorders b){zoneObject=z;area=b;}public MapZone getZone(){return zoneObject;}public ZoneBorders getBorders(){return area;}}',
    'core/game/world/map/zone/impl/MultiwayCombatZone.java': 'package core.game.world.map.zone.impl;public class MultiwayCombatZone extends core.game.world.map.zone.MapZone{private static final MultiwayCombatZone INSTANCE=new MultiwayCombatZone();private MultiwayCombatZone(){super("MultiwayCombatZone");}public static MultiwayCombatZone getInstance(){return INSTANCE;}}',
    'core/game/node/entity/player/link/music/MusicZone.java': 'package core.game.node.entity.player.link.music;public class MusicZone{private final int track;private final core.game.world.map.zone.ZoneBorders area;public MusicZone(int id,core.game.world.map.zone.ZoneBorders b){track=id;area=b;}public int getMusicId(){return track;}public core.game.world.map.zone.ZoneBorders getBorders(){return area;}}',
    'core/game/node/entity/player/Player.java': '''package core.game.node.entity.player;
import core.game.node.entity.Entity;import core.game.world.map.*;import core.game.world.map.build.DynamicRegion;
import core.net.packet.PacketRepository;import core.net.packet.context.*;import core.net.packet.out.*;
public class Player extends Entity{
 public Location lastScene;private final Flags flagsObject=new Flags(this);private final Details detailsObject=new Details();
 public Player(String name,Location l){super(name,l);}
 public core.net.IoSession getSession(){return oracle.Support.session;}
 public Details getDetails(){return detailsObject;}
 public Flags getPlayerFlags(){return flagsObject;}
 public boolean isArtificial(){return false;}
 public boolean isPlaying(){throw new UnsupportedOperationException("private ground items are outside this corpus");}
 public <T> T getAttribute(String key,T fallback){throw new UnsupportedOperationException("chunk drawing is outside this corpus");}
 public static class Details{public core.net.IoSession getSession(){return oracle.Support.session;}}
 public static class Flags{private final Player owner;Flags(Player p){owner=p;}public void setLastSceneGraph(Location l){owner.lastScene=l;}public Location getLastSceneGraph(){return owner.lastScene;}}
 /* ORIGINAL_UPDATE_SCENE_GRAPH */
}''',
    'core/game/node/entity/npc/NPC.java': '''package core.game.node.entity.npc;
import core.game.node.entity.Entity;import core.game.world.map.*;
public class NPC extends Entity{
 private final int npcId;private boolean aggressiveFlag,respawnFlag,walksFlag;
 public NPC(String name,int id,Location l,Direction d){super(name,l);npcId=id;direction=d;}
 public static NPC create(int id,Location l,Direction d){NPC n=new NPC(oracle.Support.copyName(),id,l,d);oracle.Support.emit("npcCreate",n.getName(),id,oracle.Support.coords(l),d==null?null:d.name());oracle.Support.fail("npcCreate");return n;}
 public int getId(){return npcId;}
 public boolean isAggressive(){return aggressiveFlag;}
 public boolean isRespawn(){return respawnFlag;}
 public boolean isWalks(){return walksFlag;}
 public void setAggressive(boolean v){oracle.Support.emit("npcAggressive",name,v);aggressiveFlag=v;}
 public void setRespawn(boolean v){oracle.Support.emit("npcRespawn",name,v);respawnFlag=v;}
 public void setWalks(boolean v){oracle.Support.emit("npcWalks",name,v);walksFlag=v;}
 public void init(){oracle.Support.emit("npcInit",name);oracle.Support.fail("npcInit");core.game.world.map.RegionManager.move(this,null,getLocation());}
 public void clear(){oracle.Support.emit("npcClear",name);oracle.Support.fail("npcClear");}
 public void onRegionInactivity(){oracle.Support.emit("regionInactivity",name);}
}''',
    'content/global/skill/summoning/familiar/Familiar.java': '''package content.global.skill.summoning.familiar;
import core.game.node.entity.player.Player;import core.game.world.map.*;
public class Familiar extends core.game.node.entity.npc.NPC{
 private final Player ownerObject;
 public Familiar(String name,int id,Location l,Player owner){super(name,id,l,null);ownerObject=owner;}
 public Player getOwner(){return ownerObject;}
 public void setInvisible(boolean v){oracle.Support.emit("setInvisible",name,v);oracle.Support.fail("familiar");}
 public boolean call(){throw new UnsupportedOperationException("the recall script never runs in this corpus");}
}''',
    'core/game/node/item/Item.java': 'package core.game.node.item;public class Item{public final int itemId;public Item(int id){itemId=id;}}',
    'core/game/node/item/GroundItem.java': '''package core.game.node.item;
import core.game.world.map.Location;import core.game.node.entity.player.Player;
public class GroundItem{
 public final String label;public GroundItem(String label){this.label=label;}
 public GroundItem(Item i,Location l,Player p){throw new UnsupportedOperationException("chunk drawing is outside this corpus");}
 public boolean isPrivate(){throw new UnsupportedOperationException();}
 public Player getDropper(){throw new UnsupportedOperationException();}
 public boolean droppedBy(Player p){throw new UnsupportedOperationException();}
 public boolean isActive(){throw new UnsupportedOperationException();}
 public Location getLocation(){throw new UnsupportedOperationException();}
}''',
    'core/game/node/item/GroundItemManager.java': 'package core.game.node.item;public class GroundItemManager{public static void destroy(GroundItem i){oracle.Support.emit("groundDestroy",i==null?null:i.label);}public static void create(GroundItem i){throw new UnsupportedOperationException();}}',
    'core/game/world/update/flag/chunk/ItemUpdateFlag.java': '''package core.game.world.update.flag.chunk;
public class ItemUpdateFlag extends core.game.world.update.flag.UpdateFlag<Object>{
 public static final int CONSTRUCT_TYPE=0,REMOVE_TYPE=1;
 public ItemUpdateFlag(core.game.node.item.GroundItem i,int t){super(null);throw new UnsupportedOperationException("ground item flags are outside this corpus");}
 public void write(core.net.packet.IoBuffer b){throw new UnsupportedOperationException();}
 public void writeDynamic(core.net.packet.IoBuffer b,core.game.node.entity.Entity e){throw new UnsupportedOperationException();}
 public int data(){return 0;}public int ordinal(){return 0;}
}''',
    'core/net/packet/context/BuildItemContext.java': 'package core.net.packet.context;public class BuildItemContext implements core.net.packet.Context{public BuildItemContext(core.game.node.entity.player.Player p,core.game.node.item.GroundItem i){throw new UnsupportedOperationException();}public core.game.node.entity.player.Player getPlayer(){return null;}}',
    'core/net/packet/out/ConstructGroundItem.java': 'package core.net.packet.out;public class ConstructGroundItem implements core.net.packet.OutgoingPacket<core.net.packet.context.BuildItemContext>{public static core.net.packet.IoBuffer write(core.net.packet.IoBuffer b,core.game.node.item.GroundItem i){throw new UnsupportedOperationException();}public void send(core.net.packet.context.BuildItemContext c){throw new UnsupportedOperationException();}}',
    'core/net/packet/out/ClearGroundItem.java': 'package core.net.packet.out;public class ClearGroundItem implements core.net.packet.OutgoingPacket<core.net.packet.context.BuildItemContext>{public static core.net.packet.IoBuffer write(core.net.packet.IoBuffer b,core.game.node.item.GroundItem i){throw new UnsupportedOperationException();}public void send(core.net.packet.context.BuildItemContext c){throw new UnsupportedOperationException();}}',
    'core/net/packet/PacketRepository.java': '''package core.net.packet;
public class PacketRepository{
 // OUTGOING_PACKETS holds one instance per class; PacketWriteQueue.handle
 // writes scene builds at once and queues the rest for the tick's flush.
 // The corpus records every write in call order.
 public static void send(Class<? extends OutgoingPacket> clazz,Context context){
  if(context.getPlayer().getSession()==null)return;
  OutgoingPacket p;
  try{p=(OutgoingPacket)clazz.getDeclaredConstructor().newInstance();}
  catch(ReflectiveOperationException e){throw new RuntimeException(e);}
  // The original catches and logs a failing write with its stack trace.
  try{p.send(context);}catch(Exception e){oracle.Support.emit("packetError",clazz.getSimpleName());}
 }
}''',
    'core/net/IoSession.java': '''package core.net;
import java.nio.ByteBuffer;import core.net.packet.IoBuffer;import core.net.event.GameWriteEvent;
public class IoSession{
 public void disconnect(){throw new UnsupportedOperationException("outside scope");}
 public void queue(ByteBuffer b){byte[] bytes=new byte[b.remaining()];b.get(bytes);oracle.Support.frame(bytes);}
 public void write(IoBuffer b){oracle.Support.fail("session");new GameWriteEvent(this,b).write(this,b);}
 public IoSession getIsaacPair(){return this;}
 public core.cache.crypto.ISAACCipher getOutput(){return new core.cache.crypto.ISAACCipher();}
}''',
    'core/game/system/config/XteaParser.kt': '''package core.game.system.config
class XteaParser {
    companion object {
        /* ORIGINAL_REGION_XTEA */
        /* ORIGINAL_DEFAULT_KEYS */
        /* ORIGINAL_GET_REGION_XTEA */
    }
}
''',
    'core/game/world/update/MapChunkRenderer.kt': '''package core.game.world.update
object MapChunkRenderer {
    /* ORIGINAL_BUILD_AREA_DEPTH */
    /* ORIGINAL_BUILD_AREA_SIZE */
}
''',
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
    @JvmInline
    /* ORIGINAL_LOCATION_HASH */
}
''',
}
