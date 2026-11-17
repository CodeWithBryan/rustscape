"""Adapters for the zones and region-activity oracle.

Every class here stands for a system outside this port. It records the call
through oracle.Support.emit and answers from the case's input. `/* ORIGINAL_*
*/` markers are replaced by method text taken from the pinned sources when
the oracle is generated. No adapter exposes a public field that a Kotlin
caller could read in place of an original getter.
"""

ADAPTERS = {}


def adapter(path, text):
    ADAPTERS[path] = text.lstrip('\n')


adapter('core/game/node/Node.java', '''
package core.game.node;

import core.game.node.item.Item;
import core.game.world.map.Location;

/** The members of Node that the zone classes call. */
public class Node {
	public Location getLocation() {
		return null;
	}

	public Item asItem() {
		return (Item) this;
	}

	public boolean isActive() {
		return true;
	}

	public int size() {
		return 1;
	}
}
''')

adapter('core/game/node/item/Item.java', '''
package core.game.node.item;

import core.game.node.Node;

/** An item id and amount. */
public class Item extends Node {
	private final int id;
	private final int amount;

	public Item(int id) {
		this(id, 1);
	}

	public Item(int id, int amount) {
		this.id = id;
		this.amount = amount;
	}

	public int getId() {
		return id;
	}

	public int getAmount() {
		return amount;
	}
}
''')

adapter('core/game/node/entity/Entity.java', '''
package core.game.node.entity;

import core.game.node.Node;
import core.game.node.entity.combat.CombatStyle;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneMonitor;
import core.game.world.map.zone.ZoneRestriction;
import java.util.Map;
import oracle.Support;

/**
 * Recording entity. Its state is the case's attribute map; the monitor is a
 * real ZoneMonitor. isTeleBlocked is the original text.
 */
public class Entity extends Node {
	private final int slot;
	private final Map<String, Object> state;
	private final ZoneMonitor monitor = new ZoneMonitor(this);
	public Timers timers = new Timers();

	public Entity(int slot, Map<String, Object> state) {
		this.slot = slot;
		this.state = state;
	}

	public int slot() {
		return slot;
	}

	public Map<String, Object> state() {
		return state;
	}

	public class Timers {
		public Object getTimer(String name) {
			boolean present = Support.flag(state, "teleTimer", false);
			Support.emit("timer", slot, name);
			return present ? new Object() : null;
		}
	}

	public class Locks {
		public boolean isTeleportLocked() {
			boolean locked = Support.flag(state, "teleportLocked", false);
			Support.emit("teleportLocked", slot);
			return locked;
		}
	}

	public class Properties {
		public boolean isMultiZone() {
			boolean multi = Support.flag(state, "multiZone", false);
			Support.emit("multiZone", slot);
			return multi;
		}

		public void setMultiZone(boolean multi) {
			state.put("multiZone", multi);
			Support.emit("setMultiZone", slot, multi);
		}

		public boolean isNPCWalkable() {
			boolean walkable = Support.flag(state, "npcWalkable", false);
			Support.emit("npcWalkable", slot);
			return walkable;
		}
	}

	@Override
	public Location getLocation() {
		Location location = Support.loc(state.get("location"));
		Support.emit("location", slot);
		return location;
	}

	@Override
	public boolean isActive() {
		boolean active = Support.flag(state, "active", true);
		Support.emit("active", slot);
		return active;
	}

	@Override
	public int size() {
		int size = Support.num(state, "size", 1);
		Support.emit("size", slot);
		return size;
	}

	public int getId() {
		int id = Support.num(state, "npcId", 0);
		Support.emit("id", slot);
		return id;
	}

	public ZoneMonitor getZoneMonitor() {
		return monitor;
	}

	public Locks getLocks() {
		return new Locks();
	}

	public Properties getProperties() {
		return new Properties();
	}

	public Player asPlayer() {
		return (Player) this;
	}

	public boolean isPlayer() {
		return this instanceof Player;
	}

	public void updateLocation(Location last) {
		Support.emit("entityUpdate", slot, Support.coord(last));
	}

	public boolean continueAttack(Entity target, CombatStyle style, boolean message) {
		boolean answer = Support.flag(state, "continueAttack", true);
		Support.emit("entityContinueAttack", slot, target.slot(), style == null ? null : style.name(), message);
		return answer;
	}

	public boolean isIgnoreMultiBoundaries(Entity victim) {
		boolean answer = Support.flag(state, "ignoreMulti", false);
		Support.emit("entityIgnoreMulti", slot, victim.slot());
		return answer;
	}

	@SuppressWarnings("unchecked")
	public <T> T getAttribute(String key, T fail) {
		Object stored = null;
		if (key.equals("combat-attacker") && state.get("attacker") != null) {
			stored = Support.entity(state.get("attacker"));
		} else if (key.equals("combat-time") && state.get("combatTime") != null) {
			stored = ((Number) state.get("combatTime")).longValue();
		}
		Support.emit("attribute", slot, key);
		return stored == null ? fail : (T) stored;
	}

	/* ORIGINAL_IS_TELE_BLOCKED */
}
''')

adapter('core/game/node/entity/player/Player.java', '''
package core.game.node.entity.player;

import core.game.node.entity.Entity;
import core.game.node.entity.player.info.Rights;
import core.game.node.entity.player.link.TeleportManager;
import core.game.node.item.Item;
import core.game.world.map.Location;
import java.util.Map;
import oracle.Support;

/** Recording player: every sub-object is a service of the case. */
public class Player extends Entity {
	public Player(int slot, Map<String, Object> state) {
		super(slot, state);
	}

	public boolean isArtificial() {
		boolean artificial = Support.flag(state(), "artificial", false);
		Support.emit("artificial", slot());
		return artificial;
	}

	public void sendMessage(String message) {
		Support.emit("playerMessage", slot(), message);
	}

	public String getName() {
		return "p" + slot();
	}

	public class Skull {
		public boolean isWilderness() {
			boolean wilderness = Support.flag(state(), "wilderness", false);
			Support.emit("wilderness", slot());
			return wilderness;
		}

		public int getLevel() {
			int level = Support.num(state(), "skullLevel", 0);
			Support.emit("skullLevel", slot());
			return level;
		}
	}

	public class Dispatch {
		public void sendMessage(String message) {
			Support.emit("packetMessage", slot(), message);
		}

		public void sendInterfaceConfig(int interfaceId, int child, boolean hide) {
			Support.emit("interfaceConfig", slot(), interfaceId, child, hide);
		}
	}

	public class Music {
		public boolean isPlaying() {
			boolean playing = Support.flag(state(), "playing", false);
			Support.emit("musicPlaying", slot());
			return playing;
		}

		public void playDefault() {
			Support.emit("playDefault", slot());
		}

		public void unlock(int id) {
			Support.emit("unlock", slot(), id, null);
		}

		public void unlock(int id, boolean play) {
			Support.emit("unlock", slot(), id, play);
		}
	}

	public class Box {
		public int getAmount(Item item) {
			int amount = Support.num(state(), "rum", 0);
			Support.emit("inventoryAmount", slot(), item.getId());
			return amount;
		}

		public boolean remove(Item item) {
			Support.emit("inventoryRemove", slot(), item.getId(), item.getAmount());
			return true;
		}

		public boolean containsItem(Item item) {
			throw new UnsupportedOperationException("cleanItems is outside this oracle");
		}
	}

	public class Details {
		public Rights getRights() {
			Rights rights = Rights.valueOf((String) Support.get(state(), "rights", "REGULAR_PLAYER"));
			Support.emit("rights", slot());
			return rights;
		}
	}

	public class Interfaces {
		public boolean isResizable() {
			boolean resizable = Support.flag(state(), "resizable", false);
			Support.emit("resizable", slot());
			return resizable;
		}

		public int getWindowPaneId() {
			int pane = Support.num(state(), "windowPane", 548);
			Support.emit("windowPane", slot());
			return pane;
		}

		public void restoreTabs() {
			Support.emit("restoreTabs", slot());
		}
	}

	public class Teleporter {
		public boolean send(Location location, TeleportManager.TeleportType type) {
			Support.emit("teleporterSend", slot(), Support.coord(location), type.name());
			return true;
		}
	}

	public Skull getSkullManager() {
		return new Skull();
	}

	public Dispatch getPacketDispatch() {
		return new Dispatch();
	}

	public Music getMusicPlayer() {
		return new Music();
	}

	public Box getInventory() {
		return new Box();
	}

	public Box getEquipment() {
		return new Box();
	}

	public Box getBank() {
		return new Box();
	}

	public Details getDetails() {
		return new Details();
	}

	public Interfaces getInterfaceManager() {
		return new Interfaces();
	}

	public Teleporter getTeleporter() {
		return new Teleporter();
	}
}
''')

adapter('core/game/node/entity/npc/NPC.java', '''
package core.game.node.entity.npc;

import core.game.node.entity.Entity;
import java.util.Map;
import oracle.Support;

/** Recording NPC. */
public class NPC extends Entity {
	public NPC(int slot, Map<String, Object> state) {
		super(slot, state);
	}

	public class Definition {
		public boolean hasAttackOption() {
			boolean answer = Support.flag(state(), "attackOption", true);
			Support.emit("attackOption", slot());
			return answer;
		}
	}

	public void setInvisible(boolean invisible) {
		state().put("invisible", invisible);
		Support.emit("setInvisible", slot(), invisible);
	}

	public boolean isInvisible() {
		boolean invisible = Support.flag(state(), "invisible", false);
		Support.emit("invisible", slot());
		return invisible;
	}

	public Definition getDefinition() {
		return new Definition();
	}

	public boolean shouldPreventStacking(Entity mover) {
		boolean answer = Support.flag(state(), "preventStacking", true);
		Support.emit("preventStacking", slot(), mover.slot());
		return answer;
	}

	public void onRegionInactivity() {
		Support.emit("regionInactivity", slot());
	}
}
''')

adapter('content/global/skill/summoning/familiar/Familiar.java', '''
package content.global.skill.summoning.familiar;

import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import java.util.Map;
import oracle.Support;

/** Recording familiar: an NPC with an owner. */
public class Familiar extends NPC {
	public Familiar(int slot, Map<String, Object> state) {
		super(slot, state);
	}

	public Player getOwner() {
		Player owner = (Player) Support.entity(state().get("owner"));
		Support.emit("owner", slot());
		return owner;
	}
}
''')

adapter('core/game/interaction/Option.java', '''
package core.game.interaction;

/** An opaque option; zones only pass it on. */
public class Option {
	private final String name;

	public Option(String name) {
		this.name = name;
	}

	public String getName() {
		return name;
	}
}
''')

adapter('core/game/node/entity/combat/CombatStyle.java', '''
package core.game.node.entity.combat;

/** The three styles, passed through untouched. */
public enum CombatStyle {
	MELEE, RANGE, MAGIC
}
''')

adapter('core/game/node/entity/player/link/request/RequestType.java', '''
package core.game.node.entity.player.link.request;

/** An opaque request type; zones only pass it on. */
public class RequestType {
	private final String name;

	public RequestType(String name) {
		this.name = name;
	}

	public String getName() {
		return name;
	}
}
''')

adapter('core/game/node/entity/player/link/music/MusicEntry.java', '''
package core.game.node.entity.player.link.music;

/** Named by ZoneMonitor's imports only. */
public class MusicEntry {
}
''')

adapter('core/game/node/entity/player/info/Rights.java', '''
package core.game.node.entity.player.info;

/** The three rights in their original order. */
public enum Rights {
	REGULAR_PLAYER, PLAYER_MODERATOR, ADMINISTRATOR
}
''')

adapter('core/game/node/entity/player/link/TeleportManager.java', '''
package core.game.node.entity.player.link;

/** Only the teleport type that ModeratorZone names. */
public class TeleportManager {
	public enum TeleportType {
		NORMAL
	}
}
''')

adapter('core/net/packet/PacketRepository.java', '''
package core.net.packet;

import core.net.packet.context.InterfaceContext;
import oracle.Support;

/** Recording packet sink. */
public class PacketRepository {
	public static void send(Class<?> packet, InterfaceContext context) {
		Support.emit("packet", packet.getSimpleName(), context.player.slot(), context.windowId, context.componentId, context.interfaceId, context.walkable);
	}
}
''')

adapter('core/net/packet/context/InterfaceContext.java', '''
package core.net.packet.context;

import core.game.node.entity.player.Player;

/** The constructor arguments of the original context. */
public class InterfaceContext {
	public final Player player;
	public final int windowId;
	public final int componentId;
	public final int interfaceId;
	public final boolean walkable;

	public InterfaceContext(Player player, int windowId, int componentId, int interfaceId, boolean walkable) {
		this.player = player;
		this.windowId = windowId;
		this.componentId = componentId;
		this.interfaceId = interfaceId;
		this.walkable = walkable;
	}
}
''')

adapter('core/net/packet/out/Interface.java', '''
package core.net.packet.out;

/** Packet marker. */
public class Interface {
}
''')

adapter('core/net/packet/out/CloseInterface.java', '''
package core.net.packet.out;

/** Packet marker. */
public class CloseInterface {
}
''')

adapter('core/game/world/map/path/Pathfinder.java', '''
package core.game.world.map.path;

/** isStandingIn is the original text. */
public class Pathfinder {
	/* ORIGINAL_IS_STANDING_IN */
}
''')

adapter('core/tools/RandomFunction.java', '''
package core.tools;

import java.util.Random;

/** The shared generator and normalRandDist(max, intensity): original text. */
public class RandomFunction {
	/* ORIGINAL_RANDOM_FIELD */

	/* ORIGINAL_NORMAL_RAND_DIST */
}
''')

adapter('core/game/world/map/Location.kt', '''
package core.game.world.map

/**
 * Packed location. LocationHash, the region id forms, transform, toString,
 * equals and hashCode are the original text.
 */
class Location(private var hash: LocationHash) {
    @JvmOverloads
    constructor(x: Int, y: Int, z: Int = 0) : this(
        LocationHash(x, y, (z % 4 + 4) % 4)
    )

    val x: Int get() = hash.x
    val y: Int get() = hash.y
    val z: Int get() = hash.z

    companion object {
        @JvmStatic @JvmOverloads fun create(x: Int, y: Int, z: Int = 0): Location = Location(x, y, z)
    }

    /* ORIGINAL_LOCATION_MEMBERS */

    @JvmInline
    /* ORIGINAL_LOCATION_HASH */
}
''')

adapter('core/game/world/map/RegionManager.kt', '''
package core.game.world.map

import core.api.log
import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.tools.Log
import java.util.concurrent.TimeUnit
import java.util.concurrent.locks.ReentrantLock

/**
 * forId is the original text over the original cache map and lock. The local
 * entity queries and the teleport test are services of the case.
 */
object RegionManager {
    private val REGION_CACHE: MutableMap<Int, Region> = HashMap()
    private val LOCK = ReentrantLock()

    @JvmStatic
    /* ORIGINAL_FOR_ID */

    @JvmStatic
    fun getLocalNPCs(location: Location, distance: Int): List<NPC> = oracle.Support.localNpcs(location, distance)

    @JvmStatic
    fun getLocalPlayers(location: Location): List<Player> = oracle.Support.localPlayers(location, MapDistance.RENDERING.distance)

    @JvmStatic
    fun isTeleportPermitted(loc: Location): Boolean = oracle.Support.permitted(loc)

    @JvmStatic
    fun getObject(x: Int, y: Int, z: Int, objectId: Int, type: Int): core.game.node.scenery.Scenery? =
        throw UnsupportedOperationException("scenery overrides are outside this oracle")

    @JvmStatic
    fun reset() {
        REGION_CACHE.clear()
    }

    @JvmStatic
    fun cached(): Map<Int, Region> = REGION_CACHE
}
''')

adapter('core/game/world/map/RegionChunk.java', '''
package core.game.world.map;

import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.scenery.Scenery;
import core.game.world.map.build.DynamicRegion;
import java.util.ArrayList;
import java.util.List;

/** The lists and object layers Region.java touches; clear() is the original text. */
public class RegionChunk {
	public static final int SIZE = 8;
	public static final int ARRAY_SIZE = 4;
	protected Location currentBase;
	private final List<Player> players = new ArrayList<>(20);
	private final List<NPC> npcs = new ArrayList<>(20);
	private final List<Object> updateFlags = new ArrayList<>(20);
	private List<Object> items = new ArrayList<>();
	private final Scenery[][][] statObjects = new Scenery[SIZE][SIZE][ARRAY_SIZE];
	private final Scenery[][][] objects = new Scenery[SIZE][SIZE][ARRAY_SIZE];

	public static class Flags {
		public void setLandscape(boolean[][] landscape) {
			throw new UnsupportedOperationException("map loading is outside this oracle");
		}
	}

	public RegionChunk(Location base, int rotation) {
		this.currentBase = base;
	}

	public List<Player> getPlayers() {
		return players;
	}

	public List<NPC> getNpcs() {
		return npcs;
	}

	public List<Object> updateFlags() {
		return updateFlags;
	}

	public Scenery[][][] getObjects() {
		return objects;
	}

	public Scenery[][][] statics() {
		return statObjects;
	}

	public Flags getFlags() {
		return new Flags();
	}

	/* ORIGINAL_CHUNK_CLEAR */
}
''')

adapter('core/game/world/map/build/DynamicRegion.java', '''
package core.game.world.map.build;

import core.game.world.map.Region;

/** Never constructed here: dynamic regions are outside this oracle. */
public class DynamicRegion extends Region {
	public DynamicRegion(int x, int y) {
		super(x, y);
	}
}
''')

adapter('core/game/world/map/build/LandscapeParser.java', '''
package core.game.world.map.build;

import core.game.world.map.Region;
import core.game.world.map.RegionChunk;
import java.nio.ByteBuffer;

/** Not reached: the cache of this oracle has no map archives. */
public class LandscapeParser {
	public static void parse(Region r, RegionChunk[][][] chunks, byte[][][] mapscape, ByteBuffer buffer, boolean build) {
		throw new UnsupportedOperationException("map loading is outside this oracle");
	}
}
''')

adapter('core/game/world/map/build/MapscapeParser.java', '''
package core.game.world.map.build;

import core.game.world.map.Region;
import core.game.world.map.RegionChunk;
import java.nio.ByteBuffer;

/** Not reached: the cache of this oracle has no map archives. */
public class MapscapeParser {
	public static void parse(Region r, byte[][][] mapscape, ByteBuffer buffer) {
		throw new UnsupportedOperationException("map loading is outside this oracle");
	}

	public static void clipMapscape(Region r, RegionChunk[][][] chunks, byte[][][] mapscape) {
		throw new UnsupportedOperationException("map loading is outside this oracle");
	}
}
''')

adapter('core/cache/Cache.java', '''
package core.cache;

import oracle.Support;

/** Recording cache: only the archive lookup of Region.load is answered. */
public class Cache {
	public static class Files {
		public byte[] getContainerUnpackedData(int id) {
			throw new UnsupportedOperationException("map loading is outside this oracle");
		}
	}

	public static class Index {
		public int getArchiveId(String name) {
			int id = Support.num(Support.input, "archive", -1);
			Support.emit("archiveId", name);
			return id;
		}

		public Files getCacheFile() {
			return new Files();
		}

		public byte[] getFileData(int archive, int file, int[] keys) {
			throw new UnsupportedOperationException("map loading is outside this oracle");
		}
	}

	public static Index[] getIndexes() {
		Index[] indexes = new Index[6];
		indexes[5] = new Index();
		return indexes;
	}
}
''')

adapter('core/game/node/scenery/Scenery.java', '''
package core.game.node.scenery;

import core.game.node.Node;

/** A placeholder object for the chunk layers. */
public class Scenery extends Node {
	public int getId() {
		return 0;
	}

	public int getType() {
		return 0;
	}
}
''')

adapter('core/game/node/scenery/SceneryBuilder.java', '''
package core.game.node.scenery;

/** Not reached: no scenery overrides are loaded here. */
public class SceneryBuilder {
	public static boolean remove(Scenery scenery) {
		throw new UnsupportedOperationException("scenery overrides are outside this oracle");
	}

	public static boolean add(Scenery scenery) {
		throw new UnsupportedOperationException("scenery overrides are outside this oracle");
	}
}
''')

adapter('core/game/system/communication/CommunicationInfo.java', '''
package core.game.system.communication;

/** The class Region.java logs under. */
public class CommunicationInfo {
}
''')

adapter('core/game/system/config/XteaParser.java', '''
package core.game.system.config;

/** Not reached: no landscape archive is found. */
public class XteaParser {
	public static final Keys Companion = new Keys();

	public static class Keys {
		public int[] getRegionXTEA(int regionId) {
			throw new UnsupportedOperationException("map loading is outside this oracle");
		}
	}
}
''')

adapter('core/game/world/GameWorld.java', '''
package core.game.world;

/** The pulse runner of the case. */
public class GameWorld {
	public static PulseRunner getPulser() {
		return oracle.Support.runner;
	}
}
''')

adapter('core/game/world/System.kt', '''
package core.game.world

/** PulseRunner's clock: no time passes, so no long-pulse warning is logged. */
object PulseClock {
    fun currentTimeMillis(): Long = 0
}
''')

adapter('core/integrations/grafana/Grafana.kt', '''
package core.integrations.grafana

/** Telemetry sink for PulseRunner; nothing is recorded. */
object Grafana {
    fun addPulseLength(name: String, value: Int) {}

    fun countPulse(name: String) {}

    var botPulseTime: Int = 0
    var otherPulseTime: Int = 0
}
''')

adapter('core/game/bots/GeneralBotCreator.java', '''
package core.game.bots;

import core.game.system.task.Pulse;

/** Named by PulseRunner; no bot pulse exists here. */
public class GeneralBotCreator {
	public abstract static class BotScriptPulse extends Pulse {
		public Object botScript;
	}
}
''')

adapter('core/game/world/repository/Repository.java', '''
package core.game.world.repository;

import core.game.node.entity.npc.NPC;
import oracle.Support;

/** Recording repository. */
public class Repository {
	public static void addRenderableNPC(NPC npc) {
		Support.emit("renderable", npc.slot());
	}
}
''')

adapter('core/api/ContentAPI.kt', '''
package core.api

import core.tools.Log

/** Recording log service. */
fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", type.name, message)
}
''')

adapter('core/tools/SystemLogger.kt', '''
package core.tools

/** Named by PulseRunner's imports; the Log enum is the original text. */
object SystemLogger

/* ORIGINAL_LOG_ENUM */
''')

adapter('core/ServerConstants.kt', '''
package core

import core.game.world.map.Location

/** The two constants the compiled classes read. */
class ServerConstants {
    companion object {
        @JvmField
        var HOME_LOCATION: Location? = null

        @JvmField
        var GRAFANA_LOGGING = false
    }
}
''')

adapter('core/game/world/map/zone/impl/WildernessZone.java', '''
package core.game.world.map.zone.impl;

import core.game.world.map.zone.MapZone;

/** Not the wilderness: a placeholder so the unchanged ZoneBuilder compiles. */
public final class WildernessZone extends MapZone {
	private static final WildernessZone INSTANCE = new WildernessZone();

	private WildernessZone() {
		super("Wilderness placeholder", true);
	}

	public static WildernessZone getInstance() {
		return INSTANCE;
	}
}
''')

adapter('core/game/world/map/zone/impl/DarkZone.java', '''
package core.game.world.map.zone.impl;

import core.game.world.map.zone.MapZone;

/** Not the dark zone: a placeholder so the unchanged ZoneBuilder compiles. */
public final class DarkZone extends MapZone {
	public DarkZone() {
		super("Dark zone placeholder", true);
	}
}
''')
