"""Adapters for the player death oracle.

Every class here stands for a system that is not ported. It records the call
and answers from the case's input; `/* ORIGINAL_* */` markers are replaced by
method text taken from the pinned sources when the oracle is generated.
"""

ADAPTERS = {}


def adapter(path, text):
    ADAPTERS[path] = text.lstrip('\n')


adapter('core/game/node/Node.java', '''
package core.game.node;

import core.game.interaction.DestinationFlag;
import core.game.interaction.InteractPlugin;
import core.game.node.entity.npc.NPC;
import core.game.node.item.Item;
import core.game.node.scenery.Scenery;
import core.game.world.map.Location;

/**
 * The fields the compiled classes write, the original two-argument
 * constructor, getId and the name/index accessors. Activity records its read.
 */
public class Node {
	public String label;
	public boolean active = true;
	protected String name;
	protected Location location;
	protected int index;
	protected InteractPlugin interactPlugin;
	protected DestinationFlag destinationFlag;

	public Node() {
	}

	/* ORIGINAL_NODE_CONSTRUCTOR */

	/* ORIGINAL_NODE_GET_ID */

	public boolean isActive() {
		oracle.Support.emit("isActive", label);
		return active;
	}

	/* ORIGINAL_NODE_GET_NAME */

	public Location getLocation() {
		return location;
	}

	public void setLocation(Location location) {
		this.location = location;
	}

	/* ORIGINAL_NODE_GET_INDEX */

	/* ORIGINAL_NODE_SET_INDEX */

	/* ORIGINAL_NODE_GET_DESTINATION_FLAG */
}
''')

adapter('core/game/node/scenery/Scenery.java', '''
package core.game.node.scenery;

import core.game.node.Node;

/** Named by Node.getId; no scenery exists in this oracle. */
public class Scenery extends Node {
	public int getId() {
		throw new UnsupportedOperationException();
	}
}
''')

adapter('core/game/interaction/DestinationFlag.java', '''
package core.game.interaction;

public class DestinationFlag {
	public static final DestinationFlag ITEM = new DestinationFlag();
}
''')

adapter('core/game/interaction/InteractPlugin.java', '''
package core.game.interaction;

/** Item interaction set-up; GroundItem calls setDefault. */
public class InteractPlugin {
	public InteractPlugin(Object owner) {
	}

	public void setDefault() {
	}
}
''')

adapter('core/game/interaction/OptionHandler.java', '''
package core.game.interaction;

public class OptionHandler {
}
''')

adapter('core/game/node/entity/combat/equipment/DegradableEquipment.java', '''
package core.game.node.entity.combat.equipment;

/** Item.getDropItem's lookup; not reached by this oracle. */
public class DegradableEquipment {
	public static int getDropReplacement(int id) {
		throw new UnsupportedOperationException();
	}
}
''')

adapter('com/google/errorprone/annotations/CheckReturnValue.java', '''
package com.google.errorprone.annotations;

public @interface CheckReturnValue {
}
''')

adapter('core/game/node/entity/Entity.java', '''
package core.game.node.entity;

import core.game.event.Event;
import core.game.node.Node;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneRestriction;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import java.util.HashMap;
import java.util.Map;
import oracle.Support;

/**
 * Recording entity. commenceDeath and finalizeDeath are the original text;
 * every member they, DeathTask and the grave classes reach records its call.
 */
public class Entity extends Node {
	public Map in = new HashMap();
	protected final ImpactHandler impactHandler = new ImpactHandler(this);
	public final Map<String, Object> attributes = new HashMap<>();
	public final Properties properties = new Properties();
	protected Skills skills = new Skills(this);
	public final Timers timers = new Timers();
	public final ZoneMonitor zoneMonitor = new ZoneMonitor();
	private final Animator animator = new Animator(this);
	private final WalkingQueue walkingQueue = new WalkingQueue();

	public Entity() {
	}

	public Entity(String name, Location location) {
		super(name, location);
	}

	public NPC asNpc() {
		return (NPC) this;
	}

	public core.game.node.entity.impl.PulseManager getPulseManager() {
		throw new UnsupportedOperationException("DeathTask.startDeath is outside this oracle");
	}

	public Player asPlayer() {
		return (Player) this;
	}

	public ImpactHandler getImpactHandler() {
		return impactHandler;
	}

	public Properties getProperties() {
		return properties;
	}

	public Skills getSkills() {
		return skills;
	}

	public ZoneMonitor getZoneMonitor() {
		return zoneMonitor;
	}

	public Animator getAnimator() {
		return animator;
	}

	public WalkingQueue getWalkingQueue() {
		return walkingQueue;
	}

	public Map<String, Object> getAttributes() {
		Support.emit("attributes", label);
		return attributes;
	}

	public void setAttribute(String key, Object value) {
		attributes.put(key, value);
		Support.emit("setAttribute", label, key, Support.value(value));
	}

	public void removeAttribute(String key) {
		attributes.remove(key);
		Support.emit("removeAttribute", label, key);
	}

	@SuppressWarnings("unchecked")
	public <T> T getAttribute(String key, T fallback) {
		Support.emit("getAttribute", label, key);
		Object value = attributes.get(key);
		return value == null ? fallback : (T) value;
	}

	public void incrementAttribute(String key) {
		Support.emit("incrementAttribute", label, key);
	}

	public void lock(int ticks) {
		Support.emit("lock", label, ticks);
	}

	public void unlock() {
		Support.emit("unlock", label);
	}

	public void face(Entity target) {
		Support.emit("face", label, target == null ? null : target.label);
	}

	public void graphics(Graphics graphics) {
		Support.emit("graphics", label, graphics.id);
	}

	public void visualize(Animation animation, Graphics graphics) {
		Support.emit("visualize", label, animation.id, graphics == null ? null : graphics.id);
	}

	public void dispatch(Event event) {
		Support.emit("dispatch", label, event.name(), event.subject());
	}

	/* ORIGINAL_ENTITY_COMMENCE_DEATH */

	/* ORIGINAL_ENTITY_FINALIZE_DEATH */

	public class Properties {
		public Animation deathAnimation = new Animation(836, 4);
		public Graphics deathGfx;
		public Location safeRespawn;

		public Animation getDeathAnimation() {
			Support.emit("deathAnimation", label);
			java.util.List value = (java.util.List) in.get("deathAnimation");
			return value == null ? deathAnimation : new Animation(Support.integer(value.get(0)), Support.integer(value.get(1)));
		}

		public boolean isSafeZone() {
			Support.emit("isSafeZone", label);
			return Support.flag(in, "safeZone", false);
		}

		public Location getSpawnLocation() {
			Support.emit("spawnLocation", label);
			return Support.place(in.get("spawn"));
		}

		public void setTeleportLocation(Location location) {
			Support.emit("teleport", label, Support.location(location));
		}
	}

	public class Timers {
		public void onEntityDeath() {
			Support.emit("timersOnDeath", label);
		}
	}

	public class ZoneMonitor {
		public boolean handleDeath(Entity killer) {
			Support.emit("zoneHandleDeath", label, killer == null ? null : killer.label);
			return Support.flag(in, "zoneHandle", false);
		}

		public int getType() {
			Support.emit("zoneType", label);
			return Support.integer(in, "zoneType", 0);
		}

		public boolean isRestricted(ZoneRestriction restriction) {
			Support.emit("zoneRestricted", label, restriction.name());
			return Support.flag(in, "restrictGraves", false);
		}
	}

	public class WalkingQueue {
		public void reset() {
			Support.emit("walkingReset", label);
		}
	}
}
''')

adapter('core/game/node/entity/npc/NPC.java', '''
package core.game.node.entity.npc;

import core.game.node.entity.Entity;
import core.game.world.map.Location;
import oracle.Support;

/** A recording NPC: the killers and the grave's base. */
public class NPC extends Entity {
	protected int id;

	public NPC(int id) {
		this.id = id;
	}

	public NPC(int id, Location location) {
		super(null, location);
		this.id = id;
	}

	public int getId() {
		Support.emit("npcId", label);
		return id;
	}

	public void transform(int id) {
		Support.emit("transform", label, id);
		this.id = id;
	}

	@Override
	public Location getLocation() {
		Support.emit("npcLocation", label);
		return super.getLocation();
	}

	@Override
	public void setLocation(Location location) {
		Support.emit("setLocation", label, Support.location(location));
		super.setLocation(location);
	}

	public void init() {
		Support.emit("init", label);
		in.put("ticking", true);
	}

	public core.game.node.entity.player.link.audio.Audio getAudio(int index) {
		throw new UnsupportedOperationException("a dying player never takes the NPC branch");
	}

	public int getRespawnTick() {
		throw new UnsupportedOperationException("DeathTask.isDead is outside this oracle");
	}

	public void clear() {
		Support.emit("clear", label);
		in.put("ticking", false);
	}

	public boolean isRespawn() {
		throw new UnsupportedOperationException();
	}

	public void setRespawn(boolean respawn) {
		Support.emit("setRespawn", label, respawn);
	}

	public boolean isWalks() {
		throw new UnsupportedOperationException();
	}

	public void setWalks(boolean walks) {
		Support.emit("setWalks", label, walks);
	}

	public boolean isNeverWalks() {
		throw new UnsupportedOperationException();
	}

	public void setNeverWalks(boolean neverWalks) {
		Support.emit("setNeverWalks", label, neverWalks);
	}

	public void tick() {
	}

	/** State capture without a recorded call. */
	public int stateId() {
		return id;
	}

	/** State capture without a recorded call. */
	public Location stateLocation() {
		return location;
	}
}
''')

adapter('core/game/node/entity/npc/AbstractNPC.java', '''
package core.game.node.entity.npc;

import core.game.world.map.Location;

/** The grave's base class: its constructors register the grave with the case. */
public abstract class AbstractNPC extends NPC {
	public AbstractNPC(int id, Location location, boolean autowalk) {
		super(id, location);
		oracle.Support.grave(this, id, location, autowalk);
	}

	public AbstractNPC(int id, Location location) {
		this(id, location, true);
	}

	public abstract AbstractNPC construct(int id, Location location, Object... objects);

	public abstract int[] getIds();
}
''')

adapter('core/plugin/Initializable.java', '''
package core.plugin;

import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;

@Retention(RetentionPolicy.RUNTIME)
public @interface Initializable {
}
''')

adapter('core/plugin/Plugin.java', '''
package core.plugin;

/** An item's equipment plugin: fireEvent records its arguments. */
public class Plugin {
	public final int id;

	public Plugin(int id) {
		this.id = id;
	}

	public Object fireEvent(String identifier, Object... args) {
		oracle.Support.emit("equipmentEvent", id, identifier, oracle.Support.label(args[0]), oracle.Support.item((core.game.node.item.Item) args[1]));
		return null;
	}
}
''')

adapter('core/game/node/entity/player/Player.java', '''
package core.game.node.entity.player;

import content.global.handlers.item.equipment.BarrowsEquipment;
import content.global.skill.slayer.SlayerEquipmentFlags;
import core.api.ContentAPIKt;
import core.game.component.Component;
import core.game.container.Container;
import core.game.container.ContainerType;
import core.game.interaction.InteractionListeners;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.DeathTask;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.info.LogType;
import core.game.node.entity.player.info.PlayerMonitor;
import core.game.node.entity.player.info.Rights;
import core.game.node.entity.player.link.IronmanMode;
import core.game.node.entity.player.link.prayer.PrayerType;
import core.game.node.entity.skill.Skills;
import core.game.node.entity.combat.graves.Grave;
import core.game.node.entity.combat.graves.GraveController;
import core.game.node.item.GroundItem;
import core.game.node.item.GroundItemManager;
import core.game.node.item.Item;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneType;
import core.game.world.repository.Repository;
import core.plugin.Plugin;
import core.GlobalStats;
import org.rs09.consts.Items;
import oracle.Support;
import java.util.*;

import static core.api.ContentAPIKt.*;
import static core.api.utils.PermadeathKt.permadeath;
import static core.game.system.command.sets.StatAttributeKeysKt.STATS_BASE;
import static core.game.system.command.sets.StatAttributeKeysKt.STATS_DEATHS;
import static core.tools.GlobalsKt.colorize;
import static org.rs09.consts.Items.BONES_526;

/**
 * Recording player. commenceDeath and finalizeDeath are the original text;
 * the inventory and equipment are real Containers (equipment as the base
 * class); every other member records its call and answers from the case.
 */
public class Player extends Entity {
	private final Container inventory = new Container(28);
	private final Container equipment = new Container(14);
	private final Prayer prayer = new Prayer();
	private final Settings settings = new Settings();
	private final PacketDispatch packetDispatch = new PacketDispatch();
	private final SkullManager skullManager = new SkullManager();
	private final IronmanManager ironmanManager = new IronmanManager();
	private final Details details = new Details();
	private final FamiliarManager familiarManager = new FamiliarManager();
	private final InterfaceManager interfaceManager = new InterfaceManager();
	private final Appearance appearance = new Appearance();
	private final SavedData savedData = new SavedData();

	public Player() {
	}

	public Container getInventory() {
		return inventory;
	}

	public Container getEquipment() {
		return equipment;
	}

	public Prayer getPrayer() {
		return prayer;
	}

	public PacketDispatch getPacketDispatch() {
		return packetDispatch;
	}

	public SkullManager getSkullManager() {
		return skullManager;
	}

	public IronmanManager getIronmanManager() {
		return ironmanManager;
	}

	public Details getDetails() {
		return details;
	}

	public InterfaceManager getInterfaceManager() {
		return interfaceManager;
	}

	public SavedData getSavedData() {
		return savedData;
	}

	public boolean isPlaying() {
		Support.emit("isPlaying", label);
		return Support.flag(in, "playing", true);
	}

	public boolean isArtificial() {
		Support.emit("isArtificial", label);
		return Support.flag(in, "artificial", false);
	}

	public boolean isMale() {
		Support.emit("isMale", label);
		return Support.flag(in, "male", true);
	}

	@Override
	public String getName() {
		Support.emit("name", label);
		return name;
	}

	public String getUsername() {
		Support.emit("username", label);
		return (String) in.get("username");
	}

	public void sendMessage(String message) {
		Support.emit("playerMessage", label, message);
	}

	/* ORIGINAL_PLAYER_COMMENCE_DEATH */

	/* ORIGINAL_PLAYER_FINALIZE_DEATH */

	public class Prayer {
		public boolean get(PrayerType type) {
			Support.emit("prayer", label, type.name());
			return Support.flag(in, type == PrayerType.PROTECT_ITEMS ? "protect" : "retribution", false);
		}

		public void startRetribution(Entity killer) {
			Support.emit("retribution", label, killer == null ? null : killer.label);
		}

		public void reset() {
			Support.emit("prayerReset", label);
		}
	}

	public class Settings {
		public void setSpecialEnergy(int value) {
			Support.emit("specialEnergy", label, value);
		}

		public double getRunEnergy() {
			Support.emit("runEnergy", label);
			return Support.real(in, "runEnergy", 100.0);
		}

		public void updateRunEnergy(double value) {
			Support.emit("updateRunEnergy", label, Double.doubleToRawLongBits(value) + "");
		}
	}

	public class PacketDispatch {
		public void sendMessage(String message) {
			Support.emit("dispatchMessage", label, message);
		}

		public void sendTempMusic(int id) {
			Support.emit("tempMusic", label, id);
		}
	}

	public class SkullManager {
		public boolean isSkulled() {
			Support.emit("skulled", label);
			return Support.flag(in, "skulled", false);
		}

		public boolean isWilderness() {
			Support.emit("wilderness", label);
			return Support.flag(in, "wilderness", false);
		}

		public void setSkulled(boolean skulled) {
			Support.emit("setSkulled", label, skulled);
		}
	}

	public class IronmanManager {
		public IronmanMode getMode() {
			Support.emit("ironmanMode", label);
			Object mode = in.get("mode");
			return mode == null ? null : IronmanMode.valueOf((String) mode);
		}

		public boolean isIronman() {
			Support.emit("isIronman", label);
			Object mode = in.get("mode");
			return mode != null && !"NONE".equals(mode);
		}
	}

	public class Details {
		public int getUid() {
			Support.emit("uid", label);
			return Support.integer(in, "uid", 0);
		}

		public Rights getRights() {
			Support.emit("rights", label);
			return Rights.valueOf((String) in.getOrDefault("rights", "REGULAR_PLAYER"));
		}

		public void setRights(Rights rights) {
			throw new UnsupportedOperationException("commands are outside this oracle");
		}
	}

	public class FamiliarManager {
		public void dismiss() {
			Support.emit("dismissFamiliar", label);
		}
	}

	public class InterfaceManager {
		public void openDefaultTabs() {
			Support.emit("openDefaultTabs", label);
		}

		public void open(Component component) {
			Support.emit("openComponent", label, component.id);
		}
	}

	public class Appearance {
		public void sync() {
			Support.emit("appearanceSync", label);
		}
	}

	public class SavedData {
		public GlobalData getGlobalData() {
			return new GlobalData();
		}
	}

	public class GlobalData {
		public boolean isDeathScreenDisabled() {
			Support.emit("deathScreenDisabled", label);
			return Support.flag(in, "deathScreenDisabled", false);
		}

		public void setDisableDeathScreen(boolean disabled) {
			Support.emit("setDisableDeathScreen", label, disabled);
		}
	}
}
''')

adapter('core/game/component/Component.java', '''
package core.game.component;

public class Component {
	public final int id;

	public Component(int id) {
		this.id = id;
	}
}
''')

adapter('core/game/node/entity/impl/PulseManager.java', '''
package core.game.node.entity.impl;

import core.game.system.task.Pulse;

/** Named by DeathTask.startDeath, which this oracle does not call. */
public class PulseManager {
	public void run(Pulse pulse, PulseType type) {
		throw new UnsupportedOperationException();
	}
}
''')

adapter('core/game/node/entity/combat/ImpactHandler.java', '''
package core.game.node.entity.combat;

import core.game.node.entity.Entity;
import core.game.world.GameWorld;
import java.util.HashMap;
import java.util.Map;

/**
 * The disabled-until tick and the two damage logs of the original
 * ImpactHandler; their accessors are the original text.
 */
public final class ImpactHandler {
	private final Entity entity;
	private int disabledTicks;
	private final Map<Entity, Integer> npcImpactLog = new HashMap<>();
	private final Map<Integer, Integer> playerImpactLog = new HashMap<>();
	private final ImpactQueue impactQueue = new ImpactQueue();

	public ImpactHandler(Entity entity) {
		this.entity = entity;
	}

	public ImpactQueue getImpactQueue() {
		return impactQueue;
	}

	public void manualHit(Entity source, int hit, HitsplatType type) {
		throw new UnsupportedOperationException("commands are outside this oracle");
	}

	public enum HitsplatType {
		NORMAL
	}

	/* ORIGINAL_GET_NPC_LOG */

	/* ORIGINAL_GET_PLAYER_LOG */

	/* ORIGINAL_GET_DISABLED */

	/* ORIGINAL_SET_DISABLED */

	public class ImpactQueue {
		public void clear() {
			oracle.Support.emit("clearImpactQueue", entity.label);
		}
	}
}
''')

adapter('core/game/node/entity/impl/Animator.java', '''
package core.game.node.entity.impl;

import core.game.node.entity.Entity;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;

/** Recording animator; the two reset constants have the original declarations. */
public class Animator {
	/* ORIGINAL_RESET_A */
	/* ORIGINAL_RESET_G */
	private final Entity entity;

	public Animator(Entity entity) {
		this.entity = entity;
	}

	public void forceAnimation(Animation animation) {
		oracle.Support.emit("forceAnimation", entity.label, animation.id);
	}
}
''')

adapter('core/game/world/update/flag/context/Animation.java', '''
package core.game.world.update.flag.context;

/** An animation id with a duration in ticks given by the case. */
public class Animation {
	public final int id;
	public final int duration;

	public Animation(int id) {
		this(id, 0);
	}

	public Animation(int id, int duration) {
		this.id = id;
		this.duration = duration;
	}

	public int getDuration() {
		return duration;
	}
}
''')

adapter('core/game/world/update/flag/context/Graphics.java', '''
package core.game.world.update.flag.context;

/** A graphics id. */
public class Graphics {
	public final int id;

	public Graphics(int id) {
		this.id = id;
	}
}
''')

adapter('core/game/node/entity/player/link/audio/Audio.java', '''
package core.game.node.entity.player.link.audio;

/** Named by DeathTask's NPC branch, which a player never takes. */
public class Audio {
	public final int id;

	public Audio(int id) {
		this.id = id;
	}
}
''')

adapter('core/game/node/entity/player/link/prayer/PrayerType.java', '''
package core.game.node.entity.player.link.prayer;

/** The two prayers the death paths ask about. */
public enum PrayerType {
	PROTECT_ITEMS, RETRIBUTION
}
''')

adapter('core/game/event/Event.java', '''
package core.game.event;

/** What an entity dispatches; the oracle records its class and subject. */
public interface Event {
	String name();

	String subject();
}
''')

adapter('core/game/event/SelfDeathEvent.java', '''
package core.game.event;

import core.game.node.entity.Entity;

public class SelfDeathEvent implements Event {
	private final Entity killer;

	public SelfDeathEvent(Entity killer) {
		this.killer = killer;
	}

	public String name() {
		return "SelfDeathEvent";
	}

	public String subject() {
		return killer.label;
	}
}
''')

adapter('core/game/world/GameWorld.kt', '''
package core.game.world

/** The tick counter of the case. */
object GameWorld {
    @JvmStatic
    val ticks: Int
        get() = oracle.Support.tick
}
''')

adapter('core/game/world/map/Location.kt', '''
package core.game.world.map

import core.game.node.Node
import kotlin.math.sqrt

/**
 * Coordinates with the original plane normalisation, equality, string form,
 * fromString, getDistance and withinDistance (the last four are the original
 * text).
 */
class Location(x: Int, y: Int, z: Int) {
    val x: Int = x
    val y: Int = y
    val z: Int = (z % 4 + 4) % 4

    override fun equals(other: Any?): Boolean = other is Location && other.x == x && other.y == y && other.z == z

    override fun hashCode(): Int = (z shl 30) or (x shl 15) or y

    /* ORIGINAL_LOCATION_TO_STRING */

    /* ORIGINAL_LOCATION_WITHIN */

    companion object {
        @JvmStatic fun create(x: Int, y: Int, z: Int): Location = Location(x, y, z)

        /* ORIGINAL_LOCATION_FROM_STRING */

        @JvmStatic /* ORIGINAL_LOCATION_DISTANCE */
    }
}
''')

adapter('core/game/world/map/RegionManager.java', '''
package core.game.world.map;

import oracle.Support;

/** One chunk per case. */
public class RegionManager {
	public static RegionChunk getRegionChunk(Location location) {
		if (location == null) {
			throw new NullPointerException();
		}
		Support.emit("chunk", Support.location(location));
		return Support.chunk;
	}
}
''')

adapter('core/game/world/map/RegionChunk.java', '''
package core.game.world.map;

import core.game.node.item.GroundItem;
import core.game.world.update.flag.chunk.ItemUpdateFlag;
import core.net.packet.PacketRepository;
import core.net.packet.context.BuildItemContext;
import core.net.packet.out.ClearGroundItem;
import core.net.packet.out.ConstructGroundItem;
import java.util.ArrayList;
import java.util.List;

/** The chunk's ground-item list. add(GroundItem) and remove(GroundItem) are the original text. */
public class RegionChunk {
	private final List<GroundItem> items = new ArrayList<>();
	public final List<ItemUpdateFlag> flags = new ArrayList<>();

	public List<GroundItem> getItems() {
		return items;
	}

	public void flag(ItemUpdateFlag flag) {
		flags.add(flag);
		oracle.Support.emit("flagged", flag.type, oracle.Support.item(flag.item));
	}

	/* ORIGINAL_CHUNK_ADD */

	/* ORIGINAL_CHUNK_REMOVE */
}
''')

adapter('core/game/world/update/flag/chunk/ItemUpdateFlag.java', '''
package core.game.world.update.flag.chunk;

import core.game.node.item.GroundItem;

/** A queued public ground-item update; the constants have the original declarations. */
public class ItemUpdateFlag {
	/* ORIGINAL_CONSTRUCT_TYPE */
	/* ORIGINAL_REMOVE_TYPE */
	public final GroundItem item;
	public final int type;

	public ItemUpdateFlag(GroundItem item, int type) {
		this.item = item;
		this.type = type;
	}
}
''')

adapter('core/net/packet/PacketRepository.java', '''
package core.net.packet;

import core.net.packet.context.BuildItemContext;
import core.net.packet.out.ClearGroundItem;
import core.net.packet.out.ConstructGroundItem;

/** Records the private ground-item packet instead of encoding it. */
public class PacketRepository {
	public static void send(Class<?> packet, BuildItemContext context) {
		oracle.Support.emit("packet", packet == ConstructGroundItem.class ? "construct" : packet == ClearGroundItem.class ? "clear" : "amount",
			context.player == null ? null : context.player.label, oracle.Support.item(context.item));
	}
}
''')

adapter('core/net/packet/context/BuildItemContext.java', '''
package core.net.packet.context;

import core.game.node.entity.player.Player;
import core.game.node.item.Item;

public class BuildItemContext {
	public final Player player;
	public final Item item;

	public BuildItemContext(Player player, Item item) {
		this.player = player;
		this.item = item;
	}

	public BuildItemContext(Player player, Item item, int oldAmount) {
		this(player, item);
	}
}
''')

adapter('core/net/packet/out/ConstructGroundItem.java', '''
package core.net.packet.out;

public class ConstructGroundItem {
}
''')

adapter('core/net/packet/out/ClearGroundItem.java', '''
package core.net.packet.out;

public class ClearGroundItem {
}
''')

adapter('core/net/packet/out/UpdateGroundItemAmount.java', '''
package core.net.packet.out;

public class UpdateGroundItemAmount {
}
''')

adapter('core/game/bots/AIRepository.java', '''
package core.game.bots;

import core.game.node.entity.player.Player;
import core.game.node.item.GroundItem;
import java.util.ArrayList;

/** Bot item registry; no player of this oracle is artificial when its items expire. */
public class AIRepository {
	public static ArrayList<GroundItem> getItems(Player player) {
		oracle.Support.emit("aiItems", player.label);
		return null;
	}
}
''')

adapter('core/game/node/item/ItemPlugin.java', '''
package core.game.node.item;

import core.game.node.entity.player.Player;
import oracle.Support;

/** Recording item plugin; its death item is a case input. DROP has the original declaration. */
public class ItemPlugin {
	/* ORIGINAL_PLUGIN_DROP */
	public int id;
	public Object deathItem;

	public Item getDeathItem(Item item) {
		Support.emit("deathItem", id, Support.item(item));
		return Support.replacement(deathItem, item);
	}

	public void remove(Player player, Item item, int type) {
		Support.emit("pluginRemove", id, player == null ? null : player.label, Support.item(item), type);
	}
}
''')

adapter('core/cache/def/impl/ItemDefinition.java', '''
package core.cache.def.impl;

import java.util.*;
import core.cache.def.Definition;
import core.game.node.item.*;
import core.game.interaction.OptionHandler;
import core.game.system.config.ItemConfigParser;
import core.tools.Log;
import static core.api.ContentAPIKt.log;

/**
 * The original fields and defaults with the original text of the methods the
 * compiled classes call: forId, getDefinitions, getName, isStackable,
 * getValue, getAlchemyValue, getNoteId, isUnnoted, getItemPlugin, hasAction,
 * hasDestroyAction, isTradeable, getOptionHandler.
 */
public class ItemDefinition extends Definition<Item> {
/* ORIGINAL_ITEMDEF_BODY */
}
''')

adapter('core/tools/StringUtils.java', '''
package core.tools;

/** Definition's examine text; not reached by this oracle. */
public class StringUtils {
	public static boolean isPlusN(String name) {
		throw new UnsupportedOperationException();
	}
}
''')

adapter('core/tools/SystemLogger.kt', '''
package core.tools

/* ORIGINAL_LOG_ENUM */
''')

adapter('core/api/ContentAPI.kt', '''
package core.api

import core.cache.def.impl.ItemDefinition
import core.game.component.Component
import core.game.interaction.QueueStrength
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.tools.Log
import oracle.Support

// Recording services for systems outside this oracle, one per ContentAPI call.
fun log(origin: Class<*>, type: Log, message: String) {
    Support.emit("log", origin.simpleName, type.name, message)
}

@JvmOverloads
fun playGlobalAudio(location: core.game.world.map.Location, id: Int, delay: Int = 0, loops: Int = 1, radius: Int = 8) {
    throw UnsupportedOperationException("a dying player never takes the NPC branch")
}

fun sendMessage(player: Player, message: String) {
    Support.emit("sendMessage", player.label, message)
}

fun getItemName(id: Int): String {
    Support.emit("itemName", id)
    return ItemDefinition.forId(id).name
}

fun itemDefinition(id: Int): ItemDefinition {
    Support.emit("itemDefinition", id)
    return ItemDefinition.forId(id)
}

fun getStatLevel(entity: Entity, skill: Int): Int {
    Support.emit("statLevel", entity.label, skill)
    return Support.statLevel(entity, skill)
}

fun setComponentVisibility(player: Player, iface: Int, child: Int, hide: Boolean) {
    Support.emit("componentVisibility", player.label, iface, child, hide)
}

fun <T> getAttribute(entity: Entity, attribute: String, default: T): T {
    return entity.getAttribute(attribute, default)
}

fun <T> setAttribute(entity: Entity, attribute: String, value: T) {
    entity.setAttribute(attribute, value)
}

fun setVarbit(player: Player, varbitId: Int, value: Int, save: Boolean = false) {
    Support.emit("varbit", player.label, varbitId, value)
}

fun openInterface(player: Player, id: Int) {
    Support.emit("openInterface", player.label, id)
}

fun closeInterface(player: Player) {
    Support.emit("closeInterface", player.label)
}

fun setInterfaceText(player: Player, string: String, iface: Int, child: Int) {
    Support.emit("interfaceText", player.label, string, iface, child)
}

fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1) {
    Support.emit("audio", player.label, id)
}

fun <T> animate(entity: Entity, anim: T, forced: Boolean = false) {
    Support.emit("animate", entity.label, anim)
}

fun registerHintIcon(player: Player, node: Node) {
    Support.emit("hintIcon", player.label, node.label)
}

fun clearHintIcon(player: Player) {
    Support.emit("clearHintIcon", player.label)
}

fun notify(player: Player, message: String) {
    throw UnsupportedOperationException("commands are outside this oracle")
}

fun stopExecuting(entity: Entity): Boolean {
    throw UnsupportedOperationException("commands are outside this oracle")
}

fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean) {
    throw UnsupportedOperationException("commands are outside this oracle")
}
''')

adapter('core/api/utils/Permadeath.kt', '''
package core.api.utils

import core.game.node.entity.player.Player

/** The permadeath reset is not ported: a recorded service. */
fun permadeath(target: Player) {
    oracle.Support.emit("permadeath", target.label)
}
''')

adapter('core/api/ContentInterface.kt', '''
package core.api

interface ContentInterface
''')

adapter('core/api/PersistWorld.kt', '''
package core.api

interface PersistWorld : ContentInterface {
    fun save()
    fun parse()
}
''')

adapter('core/api/TickListener.kt', '''
package core.api

interface TickListener : ContentInterface {
    fun tick()
}
''')

adapter('core/api/Commands.kt', '''
package core.api

import core.game.node.entity.player.Player
import core.game.system.command.Privilege

/** Command registration; GraveController's admin command is not exercised. */
interface Commands : ContentInterface {
    fun defineCommands()

    fun define(name: String, privilege: Privilege = Privilege.ADMIN, usage: String = "", description: String = "", handle: (Player, Array<String>) -> Unit) {
    }
}
''')

adapter('core/game/system/command/Privilege.java', '''
package core.game.system.command;

public enum Privilege {
	STANDARD, MODERATOR, ADMIN
}
''')

adapter('core/game/interaction/IntType.java', '''
package core.game.interaction;

public enum IntType {
	NPC
}
''')

adapter('core/game/interaction/QueueStrength.java', '''
package core.game.interaction;

public enum QueueStrength {
	WEAK, NORMAL, STRONG, SOFT
}
''')

adapter('core/game/interaction/InteractionListener.kt', '''
package core.game.interaction

import core.api.ContentInterface
import core.game.node.Node
import core.game.node.entity.player.Player

/** Listener registration: the handlers are kept by option for the case to invoke. */
interface InteractionListener : ContentInterface {
    fun defineListeners()

    fun on(ids: IntArray, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        for (name in option) oracle.Support.listen(name, handler)
    }
}
''')

adapter('core/game/interaction/InterfaceListener.kt', '''
package core.game.interaction

import core.api.ContentInterface
import core.game.component.Component
import core.game.node.entity.player.Player

/** Interface listener registration: the handler is kept for the case to invoke. */
interface InterfaceListener : ContentInterface {
    fun defineInterfaceListeners()

    fun on(componentID: Int, handler: (player: Player, component: Component, opcode: Int, buttonID: Int, slot: Int, itemID: Int) -> Boolean) {
        oracle.Support.listenInterface(componentID, handler)
    }
}
''')

adapter('core/game/interaction/InteractionListeners.java', '''
package core.game.interaction;

import core.game.node.Node;
import core.game.node.entity.player.Player;

/** The unequip listeners a death runs for each worn item. */
public class InteractionListeners {
	public static boolean run(int id, Player player, Node node, boolean isEquip) {
		oracle.Support.emit("unequipListeners", id, player.label, oracle.Support.item((core.game.node.item.Item) node), isEquip);
		return true;
	}
}
''')

adapter('content/global/skill/slayer/SlayerEquipmentFlags.java', '''
package content.global.skill.slayer;

import core.game.node.entity.player.Player;

public class SlayerEquipmentFlags {
	public static void updateFlags(Player player) {
		oracle.Support.emit("slayerFlags", player.label);
	}
}
''')

adapter('core/game/node/entity/player/info/PlayerMonitor.java', '''
package core.game.node.entity.player.info;

import core.game.node.entity.player.Player;

public class PlayerMonitor {
	public static void log(Player player, LogType type, String details) {
		oracle.Support.emit("monitor", player.label, type.name(), details);
	}
}
''')

adapter('core/game/node/entity/player/info/LogType.java', '''
package core.game.node.entity.player.info;

public enum LogType {
	PK
}
''')

adapter('core/GlobalStats.java', '''
package core;

public class GlobalStats {
	public static void incrementDeathCount() {
		oracle.Support.emit("deathCount");
	}
}
''')

adapter('core/game/world/repository/Repository.kt', '''
package core.game.world.repository

import core.game.node.entity.player.Player

/** The online players by uid, and the news line. */
object Repository {
    @JvmStatic
    val uid_map: MutableMap<Int, Player> = oracle.Support.UidMap()

    @JvmStatic
    fun sendNews(message: String) {
        oracle.Support.emit("repositoryNews", message)
    }
}
''')

adapter('core/game/world/map/zone/impl/WildernessZone.java', '''
package core.game.world.map.zone.impl;

import core.game.node.entity.Entity;

/** WildernessZone is not ported; isInZone is a case input. */
public class WildernessZone {
	public static boolean isInZone(Entity entity) {
		oracle.Support.emit("wildernessZone", entity.label);
		return oracle.Support.flag(entity.in, "inWilderness", false);
	}
}
''')

adapter('core/ServerStore.kt', '''
package core

import org.json.simple.JSONObject

/** The server store archives of the case. */
class ServerStore {
    companion object {
        @JvmStatic
        fun getArchive(name: String): JSONObject {
            oracle.Support.emit("archive", name)
            return oracle.Support.archive(name)
        }
    }
}
''')

adapter('core/game/node/entity/skill/Skills.java', '''
package core.game.node.entity.skill;

import core.game.node.entity.Entity;
import oracle.Support;

/** Recording skills. PRAYER and MAGIC are checked against the original declarations at generation time. */
public class Skills {
	public static final int PRAYER = 5;
	public static final int MAGIC = 6;
	private final Entity entity;

	public Skills(Entity entity) {
		this.entity = entity;
	}

	public void restore() {
		Support.emit("restore", entity.label);
	}

	public void rechargePrayerPoints() {
		Support.emit("rechargePrayer", entity.label);
	}

	public int getLifepoints() {
		throw new UnsupportedOperationException("commands are outside this oracle");
	}

	public int getTotalLevel() {
		Support.emit("totalLevel", entity.label);
		return Support.integer(entity.in, "totalLevel", 0);
	}

	public double getPrayerPoints() {
		Support.emit("prayerPoints", entity.label);
		return Support.prayerPoints(entity);
	}

	public void setPrayerPoints(double value) {
		Support.emit("setPrayerPoints", entity.label, Double.doubleToRawLongBits(value) + "");
		Support.setPrayerPoints(entity, value);
	}
}
''')
