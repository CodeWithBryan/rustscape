"""Adapters for the NPC death and drop oracle.

Every class here stands for a system that is not ported. It records the call
and answers from the case's input; `/* ORIGINAL_* */` markers are replaced by
method text taken from the pinned sources when the oracle is generated.
"""

ADAPTERS = {}


def adapter(path, text):
    ADAPTERS[path] = text.lstrip('\n')


adapter('core/game/node/Node.java', '''
package core.game.node;

import core.game.world.map.Location;

/** Identity, activity and the fields GroundItem's constructor writes. hashCode is a case input. */
public class Node {
	public String label;
	public boolean active = true;
	public int hash = oracle.Support.nextHash();
	protected String name;
	protected Location location;
	protected int index;
	protected Interact interactPlugin = new Interact();

	public boolean isActive() {
		return active;
	}

	public String getName() {
		return name;
	}

	public Location getLocation() {
		return location;
	}

	@Override
	public int hashCode() {
		return hash;
	}

	public static class Interact {
		public void setDefault() {
		}
	}
}
''')

adapter('core/game/node/entity/Entity.java', '''
package core.game.node.entity;

import core.game.event.Event;
import core.game.node.Node;
import core.game.node.entity.combat.CombatPulse;
import core.game.node.entity.combat.DeathTask;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.impl.Animator;
import core.game.node.entity.impl.PulseManager;
import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import java.util.HashMap;
import java.util.Map;
import oracle.Support;

/**
 * Recording entity. commenceDeath, finalizeDeath and startDeath are the
 * original text; every member they and DeathTask reach records its call.
 */
public class Entity extends Node {
	public final Scripts scripts = new Scripts();
	private final PulseManager pulseManager = new PulseManager(this);
	protected final ImpactHandler impactHandler = new ImpactHandler(this);
	public final Map<String, Object> attributes = new HashMap<>();
	public final Properties properties = new Properties();
	public final Skills skills = new Skills(this);
	public final Timers timers = new Timers();
	public final ZoneMonitor zoneMonitor = new ZoneMonitor();
	private final Animator animator = new Animator(this);
	private final WalkingQueue walkingQueue = new WalkingQueue();
	public int id;
	public int lifepoints = 1;
	public boolean zoneStart = true;
	public boolean zoneHandle;

	public int getId() {
		return id;
	}

	public NPC asNpc() {
		return (NPC) this;
	}

	public Player asPlayer() {
		return (Player) this;
	}

	public PulseManager getPulseManager() {
		return pulseManager;
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

	public void setAttribute(String key, Object value) {
		attributes.put(key, value);
		Support.emit("setAttribute", label, key, value);
	}

	public void removeAttribute(String key) {
		attributes.remove(key);
		Support.emit("removeAttribute", label, key);
	}

	@SuppressWarnings("unchecked")
	public <T> T getAttribute(String key, T fallback) {
		Object value = attributes.get(key);
		return value == null ? fallback : (T) value;
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

	public void fullRestore() {
		Support.emit("fullRestore", label);
	}

	public void tick() {
		Support.emit("entityTick", label);
	}

	/* ORIGINAL_ENTITY_COMMENCE_DEATH */

	/* ORIGINAL_ENTITY_FINALIZE_DEATH */

	/* ORIGINAL_ENTITY_START_DEATH */

	public static class Scripts {
		public void removeWeakScripts() {
		}

		public void removeNormalScripts() {
		}
	}

	public class Properties {
		public Animation deathAnimation = new Animation(836, 4);
		public Graphics deathGfx;
		public boolean safeZone;
		public boolean multiZone;
		public Location safeRespawn;
		public Location spawnLocation;
		private final CombatPulse combatPulse = new CombatPulse(Entity.this);

		public Animation getDeathAnimation() {
			return deathAnimation;
		}

		public boolean isSafeZone() {
			return safeZone;
		}

		public Location getSpawnLocation() {
			return spawnLocation;
		}

		public void setTeleportLocation(Location location) {
			Support.emit("teleportLocation", label, Support.location(location));
		}

		public CombatPulse getCombatPulse() {
			return combatPulse;
		}

		public boolean isMultiZone() {
			Support.emit("multiZone", label);
			return multiZone;
		}
	}

	public class Timers {
		public void onEntityDeath() {
			Support.emit("timersDeath", label);
		}
	}

	public class ZoneMonitor {
		public boolean startDeath(Entity entity, Entity killer) {
			Support.emit("zoneStartDeath", entity.label, killer == null ? null : killer.label);
			return zoneStart;
		}

		public boolean handleDeath(Entity killer) {
			Support.emit("zoneHandleDeath", label, killer.label);
			return zoneHandle;
		}
	}

	public class WalkingQueue {
		public void reset() {
			Support.emit("walkReset", label);
		}
	}
}
''')

adapter('core/game/node/entity/npc/NPC.java', '''
package core.game.node.entity.npc;

import static core.game.system.command.sets.StatAttributeKeysKt.STATS_BASE;
import static core.game.system.command.sets.StatAttributeKeysKt.STATS_ENEMIES_KILLED;

import core.api.utils.PlayerStatsCounter;
import core.cache.def.impl.NPCDefinition;
import core.game.event.NPCKillEvent;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.entity.player.link.audio.Audio;
import core.game.system.config.NPCConfigParser;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import oracle.Support;

/**
 * The dying NPC. tick, onRespawn, finalizeDeath, commenceDeath, handleDrops,
 * getAudio, getDropLocation, isRespawn and the respawn-tick accessors are the
 * original text; the fields they use have the original declarations.
 */
public class NPC extends Entity {
	public NPCBehavior behavior = new NPCBehavior();
	public NPCDefinition definition;
	public int originalId;
	private boolean respawn = true;
	public boolean isRespawning = false;
	private int respawnTick = -1;

	public NPC(int id) {
		this.id = id;
		this.originalId = id;
	}

	public NPCDefinition getDefinition() {
		return definition;
	}

	public void setRespawn(boolean respawn) {
		this.respawn = respawn;
	}

	@Override
	public Location getLocation() {
		Support.emit("npcLocation", label);
		return location;
	}

	public void setLocation(Location location) {
		this.location = location;
	}

	public void clear() {
		Support.emit("clear", label);
		active = false;
	}

	public void transform(int id) {
		Support.emit("transform", label, id);
	}

	public void teleport(Location location) {
		Support.emit("teleport", label, Support.location(location));
	}

	public void handleTickActions() {
		Support.emit("tickActions", label);
	}

	public void onRegionInactivity() {
		Support.emit("inactivity", label);
	}

	/* ORIGINAL_NPC_TICK */

	/* ORIGINAL_NPC_ON_RESPAWN */

	/* ORIGINAL_NPC_FINALIZE_DEATH */

	/* ORIGINAL_NPC_COMMENCE_DEATH */

	/* ORIGINAL_NPC_HANDLE_DROPS */

	/* ORIGINAL_NPC_GET_AUDIO */

	/* ORIGINAL_NPC_GET_DROP_LOCATION */

	/* ORIGINAL_NPC_IS_RESPAWN */

	/* ORIGINAL_NPC_GET_RESPAWN_TICK */

	/* ORIGINAL_NPC_SET_RESPAWN_TICK */
}
''')

adapter('core/game/node/entity/npc/NPCBehavior.kt', '''
package core.game.node.entity.npc

import core.game.node.entity.Entity
import core.game.node.item.Item

/** Recording behaviour; the signatures are the original's. */
open class NPCBehavior {
    open fun onRespawn(self: NPC) {
        oracle.Support.emit("onRespawn", self.label)
    }

    open fun onDeathStarted(self: NPC, killer: Entity) {
        oracle.Support.emit("onDeathStarted", self.label, killer.label)
    }

    open fun onDeathFinished(self: NPC, killer: Entity) {
        oracle.Support.emit("onDeathFinished", self.label, killer.label)
    }

    open fun onDropTableRolled(self: NPC, killer: Entity, drops: ArrayList<Item>) {
        oracle.Support.rolled(self, killer, drops)
    }
}
''')

adapter('core/game/node/entity/player/Player.java', '''
package core.game.node.entity.player;

import core.game.bots.GeneralBotCreator;
import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.player.link.IronmanMode;
import core.game.node.entity.player.link.prayer.PrayerType;
import core.game.node.item.GroundItem;
import core.game.world.map.Location;
import java.util.ArrayList;
import java.util.List;
import oracle.Support;

/** Recording player: every getter the drop and death paths call records itself and answers from the case. */
public class Player extends Entity {
	// Not named username: Kotlin would read a public field of that name instead of calling getUsername().
	public String accountName;
	public int uid;
	public Clan clan;
	public boolean lootShare;
	public int lootRequirement;
	public boolean ironman;
	public int lootSharePoints;
	public boolean resizable;
	public boolean boneCrusher;
	public boolean wealth;
	public List<Long> equipped = new ArrayList<>();
	public GeneralBotCreator.BotScriptPulse bot;
	private final Communication communication = new Communication();
	private final IronmanManager ironmanManager = new IronmanManager();
	private final GlobalData globalData = new GlobalData();
	private final InterfaceManager interfaceManager = new InterfaceManager();
	private final Details details = new Details();

	@Override
	public Location getLocation() {
		Support.emit("playerLocation", label);
		return location;
	}

	public void setLocation(Location location) {
		this.location = location;
	}

	public String getUsername() {
		Support.emit("username", label);
		return accountName;
	}

	public void sendMessage(String message) {
		Support.emit("message", label, message);
	}

	public void incrementAttribute(String key) {
		Support.emit("incrementAttribute", label, key);
	}

	@Override
	@SuppressWarnings("unchecked")
	public <T> T getAttribute(String key, T fallback) {
		if (key.equals("botting:script")) {
			Support.emit("botScript", label);
			return bot == null ? fallback : (T) bot;
		}
		return super.getAttribute(key, fallback);
	}

	@Override
	@SuppressWarnings("unchecked")
	public void setAttribute(String key, Object value) {
		if (key.equals("botting:drops")) {
			List<GroundItem> items = (List<GroundItem>) value;
			attributes.put(key, value);
			Support.emit("botDrop", label, Support.item(items.get(items.size() - 1)));
			return;
		}
		super.setAttribute(key, value);
	}

	@Override
	public void commenceDeath(Entity killer) {
		Support.emit("playerCommenceDeath", label, killer.label);
	}

	@Override
	public void finalizeDeath(Entity killer) {
		Support.emit("playerFinalizeDeath", label, killer.label);
	}

	public boolean isArtificial() {
		return false;
	}

	public Communication getCommunication() {
		return communication;
	}

	public IronmanManager getIronmanManager() {
		return ironmanManager;
	}

	public GlobalData getGlobalData() {
		return globalData;
	}

	public InterfaceManager getInterfaceManager() {
		return interfaceManager;
	}

	public Details getDetails() {
		return details;
	}

	public PacketDispatch getPacketDispatch() {
		return new PacketDispatch();
	}

	// Reached only by DeathTask.getContainers, which this oracle does not call.
	public Container getInventory() {
		throw new UnsupportedOperationException();
	}

	public Container getEquipment() {
		throw new UnsupportedOperationException();
	}

	public SkullManager getSkullManager() {
		throw new UnsupportedOperationException();
	}

	public Prayer getPrayer() {
		throw new UnsupportedOperationException();
	}

	public enum Rank {
		R0, R1, R2, R3, R4, R5, R6, R7, R8, R9
	}

	public static class Clan {
		public int index;
		public int lootRequirement;

		public Rank getLootRequirement() {
			Support.emit("clanReq", index);
			return Rank.values()[lootRequirement];
		}
	}

	public class Communication {
		public Clan getClan() {
			Support.emit("clan", label);
			return clan;
		}

		public boolean isLootShare() {
			Support.emit("lootShare", label);
			return lootShare;
		}

		public Rank getLootRequirement() {
			Support.emit("lootReq", label);
			return Rank.values()[lootRequirement];
		}
	}

	public class IronmanManager {
		public boolean isIronman() {
			Support.emit("ironman", label);
			return ironman;
		}

		public IronmanMode getMode() {
			throw new UnsupportedOperationException();
		}
	}

	public class GlobalData {
		public int getLootSharePoints() {
			Support.emit("lsp", label);
			return lootSharePoints;
		}

		public void setLootSharePoints(int points) {
			Support.emit("setLsp", label, points);
			lootSharePoints = points;
		}

		public boolean isEnableBoneCrusher() {
			Support.emit("boneCrusher", label);
			return boneCrusher;
		}
	}

	public class InterfaceManager {
		public boolean isResizable() {
			Support.emit("resizable", label);
			return resizable;
		}
	}

	public class Details {
		public int getUid() {
			Support.emit("uid", label);
			return uid;
		}
	}

	public class PacketDispatch {
		public void sendMessage(String message) {
			Support.emit("message", label, message);
		}
	}

	public static class SkullManager {
		public boolean isSkulled() {
			throw new UnsupportedOperationException();
		}
	}

	public static class Prayer {
		public boolean get(PrayerType type) {
			throw new UnsupportedOperationException();
		}
	}
}
''')

adapter('core/game/bots/AIPlayer.java', '''
package core.game.bots;

import core.game.node.entity.player.Player;

/** An artificial player: only its class matters to the drop and death paths. */
public class AIPlayer extends Player {
}
''')

adapter('core/game/bots/AIRepository.java', '''
package core.game.bots;

import core.game.node.entity.player.Player;
import core.game.node.item.GroundItem;
import java.util.ArrayList;

/** Recording bot item registry. */
public class AIRepository {
	public static void addItem(GroundItem item) {
		oracle.Support.emit("aiAdd", oracle.Support.item(item));
	}

	public static ArrayList<GroundItem> getItems(Player player) {
		return null;
	}
}
''')

adapter('core/game/bots/GeneralBotCreator.java', '''
package core.game.bots;

import core.game.system.task.Pulse;

/** The bot script pulse createDrop looks for; running is a case input. */
public class GeneralBotCreator {
	public static class BotScriptPulse extends Pulse {
		public Object botScript = new Object();

		@Override
		public boolean pulse() {
			return true;
		}
	}
}
''')

adapter('core/game/node/entity/combat/ImpactHandler.java', '''
package core.game.node.entity.combat;

import core.game.bots.AIPlayer;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.world.GameWorld;
import core.game.world.repository.Repository;
import java.util.HashMap;
import java.util.Map;

/**
 * The damage logs and kill credit of the original ImpactHandler. The field
 * declarations, getMostDamageEntity, the log getters and the disabled-tick
 * accessors are the original text. impact() holds the two parts of
 * handleImpact this oracle exercises, also as original text: the statement
 * that writes the logs and the branch taken at zero lifepoints. The rest of
 * handleImpact is outside this oracle.
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

	public void impact(Entity source, int hit) {
		/* ORIGINAL_HIT_LOG */
		boolean dead = false;
		/* ORIGINAL_DEATH_BRANCH */
	}

	public ImpactQueue getImpactQueue() {
		return impactQueue;
	}

	/* ORIGINAL_GET_MOST_DAMAGE */

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

adapter('core/game/node/entity/combat/CombatPulse.java', '''
package core.game.node.entity.combat;

import core.game.node.entity.Entity;
import core.game.system.task.Pulse;

/** Recording combat pulse: only stop() is reached. */
public class CombatPulse extends Pulse {
	private final Entity entity;

	public CombatPulse(Entity entity) {
		this.entity = entity;
	}

	@Override
	public boolean pulse() {
		return true;
	}

	@Override
	public void stop() {
		super.stop();
		oracle.Support.emit("stopCombat", entity.label);
	}
}
''')

adapter('core/game/interaction/MovementPulse.java', '''
package core.game.interaction;

import core.game.system.task.Pulse;

/** Named by PulseManager.isMovingPulse; never created here. */
public abstract class MovementPulse extends Pulse {
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

/** The id NPC.getAudio wraps. */
public class Audio {
	public final int id;

	public Audio(int id) {
		this.id = id;
	}
}
''')

adapter('core/game/node/entity/player/link/IronmanMode.java', '''
package core.game.node.entity.player.link;

/** Named by DeathTask.getContainers, which this oracle does not call. */
public enum IronmanMode {
	NONE, STANDARD, HARDCORE, ULTIMATE
}
''')

adapter('core/game/node/entity/player/link/prayer/PrayerType.java', '''
package core.game.node.entity.player.link.prayer;

/** Named by DeathTask.getContainers, which this oracle does not call. */
public enum PrayerType {
	PROTECT_ITEMS
}
''')

adapter('core/game/container/ContainerType.java', '''
package core.game.container;

/** Named by DeathTask.getContainers, which this oracle does not call. */
public enum ContainerType {
	DEFAULT, ALWAYS_STACK, NEVER_STACK
}
''')

adapter('core/game/container/Container.java', '''
package core.game.container;

import core.game.node.item.Item;

/** Compile-only: DeathTask.getContainers is outside this oracle. */
public class Container {
	public Container(int capacity, ContainerType type) {
		throw new UnsupportedOperationException();
	}

	public void addAll(Container container) {
		throw new UnsupportedOperationException();
	}

	public Item get(int slot) {
		throw new UnsupportedOperationException();
	}

	public Item replace(Item item, int slot) {
		throw new UnsupportedOperationException();
	}

	public boolean add(Item item, boolean fire) {
		throw new UnsupportedOperationException();
	}

	// WeightBasedTable.canRoll, which this oracle does not call.
	public boolean hasSpaceFor(Item... items) {
		throw new UnsupportedOperationException();
	}

	public boolean isFull() {
		throw new UnsupportedOperationException();
	}
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

adapter('core/game/event/NPCKillEvent.java', '''
package core.game.event;

import core.game.node.entity.npc.NPC;

public class NPCKillEvent implements Event {
	private final NPC npc;

	public NPCKillEvent(NPC npc) {
		this.npc = npc;
	}

	public String name() {
		return "NPCKillEvent";
	}

	public String subject() {
		return npc.label;
	}
}
''')

adapter('core/game/world/GameWorld.java', '''
package core.game.world;

/** The tick counter and the pulse runner of the case. */
public class GameWorld {
	public static int getTicks() {
		return oracle.Support.tick;
	}

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

adapter('core/game/world/map/Location.kt', '''
package core.game.world.map

import kotlin.math.sqrt

/** Coordinates. Both getDistance functions are the original text. */
class Location(val x: Int, val y: Int, val z: Int) {
    /* ORIGINAL_LOCATION_DISTANCE_MEMBER */

    fun getRegion(): Region = Region.INSTANCE

    override fun equals(other: Any?): Boolean = other is Location && other.x == x && other.y == y && other.z == z

    override fun hashCode(): Int = (z shl 30) or (x shl 15) or y

    companion object {
        @JvmStatic /* ORIGINAL_LOCATION_DISTANCE */
    }
}
''')

adapter('core/game/world/map/Region.java', '''
package core.game.world.map;

/** Every region of a case is active. */
public class Region {
	public static final Region INSTANCE = new Region();

	public boolean isActive() {
		return true;
	}
}
''')

adapter('core/game/world/map/RegionManager.java', '''
package core.game.world.map;

import core.game.node.entity.player.Player;
import java.util.List;
import oracle.Support;

/** One chunk per case and the case's list of local players. */
public class RegionManager {
	public static RegionChunk getRegionChunk(Location location) {
		if (location == null) {
			throw new NullPointerException();
		}
		Support.emit("chunk", Support.location(location));
		return Support.chunk;
	}

	public static List<Player> getLocalPlayers(Location location, int distance) {
		Support.emit("localPlayers", Support.location(location), distance);
		return Support.local;
	}
}
''')

adapter('core/game/world/map/RegionChunk.java', '''
package core.game.world.map;

import core.game.node.item.GroundItem;
import core.game.world.update.flag.chunk.ItemUpdateFlag;
import core.net.packet.PacketRepository;
import core.net.packet.context.BuildItemContext;
import core.net.packet.out.ConstructGroundItem;
import java.util.ArrayList;
import java.util.List;

/** The chunk's ground-item list. add(GroundItem) is the original text. */
public class RegionChunk {
	private final List<GroundItem> items = new ArrayList<>();
	public final List<ItemUpdateFlag> flags = new ArrayList<>();

	public List<GroundItem> getItems() {
		return items;
	}

	public void flag(ItemUpdateFlag flag) {
		flags.add(flag);
		oracle.Support.emit("flagged");
	}

	public void remove(GroundItem item) {
		throw new UnsupportedOperationException("ground-item removal is outside this oracle");
	}

	/* ORIGINAL_CHUNK_ADD */
}
''')

adapter('core/game/world/update/flag/chunk/ItemUpdateFlag.java', '''
package core.game.world.update.flag.chunk;

import core.game.node.item.GroundItem;

/** A queued public ground-item update; the constant has the original declaration. */
public class ItemUpdateFlag {
	/* ORIGINAL_CONSTRUCT_TYPE */
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
import core.net.packet.out.ConstructGroundItem;

/** Records the private ground-item packet instead of encoding it. */
public class PacketRepository {
	public static void send(Class<?> packet, BuildItemContext context) {
		oracle.Support.emit("packet", packet == ConstructGroundItem.class ? "construct" : "amount",
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

adapter('core/net/packet/out/UpdateGroundItemAmount.java', '''
package core.net.packet.out;

public class UpdateGroundItemAmount {
}
''')

adapter('core/game/node/item/Item.java', '''
package core.game.node.item;

import core.cache.def.impl.ItemDefinition;
import core.game.node.Node;

/**
 * The original Item's state: the packed id and charge, the amount and the
 * definition, set as its three-argument constructor sets them. The accessors
 * are the original text. Node, interaction and destination set-up are not
 * exercised.
 */
public class Item extends Node {
	private int idHash;
	private int amount;
	private ItemDefinition definition;

	public Item(int id) {
		this(id, 1, 1000);
	}

	public Item(int id, int amount) {
		this(id, amount, 1000);
	}

	public Item(int id, int amount, int charge) {
		this.name = ItemDefinition.forId(id).getName();
		this.index = -1;
		this.idHash = id << 16 | charge;
		this.amount = amount;
		this.definition = ItemDefinition.forId(id);
	}

	/* ORIGINAL_ITEM_NOTE_CHANGE */

	/* ORIGINAL_ITEM_GET_ID */

	/* ORIGINAL_ITEM_GET_AMOUNT */

	/* ORIGINAL_ITEM_SET_AMOUNT */

	/* ORIGINAL_ITEM_GET_CHARGE */

	/* ORIGINAL_ITEM_HAS_PLUGIN */

	/* ORIGINAL_ITEM_GET_PLUGIN */

	/* ORIGINAL_ITEM_GET_DEFINITION */
}
''')

adapter('core/game/node/item/ItemPlugin.java', '''
package core.game.node.item;

import core.game.node.entity.npc.NPC;
import core.game.node.entity.player.Player;
import core.game.world.map.Location;
import oracle.Support;

/** Recording item plugin; its answers are case inputs. DROP has the original declaration. */
public class ItemPlugin {
	/* ORIGINAL_PLUGIN_DROP */
	public int id;
	public boolean create;
	public Object replacement;

	public boolean createDrop(Item item, Player player, NPC npc, Location location) {
		Support.emit("pluginCreateDrop", id, Support.item(item), player.label, npc.label, Support.location(location));
		return create;
	}

	public Item getItem(Item item, NPC npc) {
		Support.emit("pluginGetItem", id, Support.item(item), npc.label);
		return Support.replacement(replacement, item);
	}

	public void remove(Player player, Item item, int type) {
		Support.emit("pluginRemove", id, player == null ? null : player.label, Support.item(item), type);
	}
}
''')

adapter('core/cache/def/impl/ItemDefinition.java', '''
package core.cache.def.impl;

import core.game.node.item.ItemPlugin;
import core.game.system.config.ItemConfigParser;
import java.util.HashMap;
import java.util.Map;

/**
 * An item definition whose fields are case inputs. isStackable, isUnnoted,
 * getNoteId, isTradeable, hasAction, hasDestroyAction, getConfiguration and
 * the body of getItemPlugin are the original text.
 */
public class ItemDefinition {
	public int id;
	public String name = "null";
	public boolean stackable;
	public boolean unnoted = true;
	public int noteId = -1;
	public String[] options = {null, null, null, null, "drop"};
	public final Map<String, Object> handlers = new HashMap<>();

	public static ItemDefinition forId(int id) {
		return oracle.Support.itemDefinition(id);
	}

	public String getName() {
		return name;
	}

	public int getAlchemyValue(boolean high) {
		throw new UnsupportedOperationException();
	}

	public ItemPlugin getItemPlugin() {
		oracle.Support.emit("plugin", id);
		return itemPluginBody();
	}

	/* ORIGINAL_ITEMDEF_PLUGIN */

	/* ORIGINAL_ITEMDEF_STACKABLE */

	/* ORIGINAL_ITEMDEF_UNNOTED */

	/* ORIGINAL_ITEMDEF_NOTE_ID */

	/* ORIGINAL_ITEMDEF_TRADEABLE */

	/* ORIGINAL_ITEMDEF_HAS_ACTION */

	/* ORIGINAL_ITEMDEF_DESTROY */

	@SuppressWarnings("unchecked")
	/* ORIGINAL_GET_CONFIGURATION */
}
''')

adapter('core/cache/def/impl/NPCDefinition.java', '''
package core.cache.def.impl;

import core.game.node.entity.npc.drop.NPCDropTables;
import java.util.HashMap;
import java.util.Map;

/**
 * An NPC definition whose handlers are case inputs. The dropTables field
 * declaration, its accessors and getConfiguration are the original text.
 */
public class NPCDefinition {
	public final int id;
	public String name;
	public int combatLevel;
	public final Map<String, Object> handlers = new HashMap<>();
	/* ORIGINAL_NPCDEF_DROP_TABLES */

	public NPCDefinition(int id) {
		this.id = id;
	}

	public String getName() {
		return name;
	}

	public int getCombatLevel() {
		return combatLevel;
	}

	public Map<String, Object> getHandlers() {
		return handlers;
	}

	/* ORIGINAL_NPCDEF_GET_DROP_TABLES */

	/* ORIGINAL_NPCDEF_SET_DROP_TABLES */

	@SuppressWarnings("unchecked")
	/* ORIGINAL_GET_CONFIGURATION */
}
''')

adapter('core/api/ContentAPI.kt', '''
package core.api

import core.api.utils.PlayerStatsCounter
import core.game.node.entity.player.Player
import core.game.node.item.Item
import core.game.system.config.ItemConfigParser
import core.game.world.map.Location
import core.tools.Log
import java.io.PrintWriter
import java.io.StringWriter

// Recording services for systems outside this oracle. announceIfRare and
// exceptionToString are the original text, inserted at generation time.
fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", origin.simpleName, type.name, message)
}

fun shouldRemoveNothings(player: Player): Boolean {
    oracle.Support.emit("wealth", player.label)
    return player.wealth
}

fun inEquipment(player: Player, id: Int, amount: Int = 1): Boolean {
    oracle.Support.emit("equipped", player.label, id)
    return player.equipped.contains(id.toLong())
}

fun sendNews(message: String) {
    oracle.Support.emit("news", message)
}

@JvmOverloads
fun playGlobalAudio(location: Location, id: Int, delay: Int = 0, loops: Int = 1, radius: Int = 8) {
    oracle.Support.emit("globalAudio", oracle.Support.location(location), id)
}

/* ORIGINAL_ANNOUNCE_IF_RARE */

/* ORIGINAL_EXCEPTION_TO_STRING */
''')

adapter('core/api/utils/PlayerStatsCounter.java', '''
package core.api.utils;

import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import oracle.Support;

/** Recording statistics counter. */
public class PlayerStatsCounter {
	public static void incrementKills(Player player, int npcId) {
		Support.emit("incrementKills", player.label, npcId);
	}

	public static void incrementRareDrop(Player player, Item item) {
		Support.emit("rareDrop", player.label, Support.item(item));
	}
}
''')

adapter('core/tools/SystemLogger.kt', '''
package core.tools

/** Named by PulseRunner's imports; the Log enum below is the original text. */
object SystemLogger

/* ORIGINAL_LOG_ENUM */
''')

adapter('core/tools/StringUtils.java', '''
package core.tools;

import java.text.DecimalFormat;

/** getFormattedNumber is the original text. */
public class StringUtils {
	/* ORIGINAL_FORMATTED_NUMBER */
}
''')

adapter('core/ServerConstants.kt', '''
package core

/** The fields the compiled classes read; declarations mirror the original companion. */
class ServerConstants {
    companion object {
        @JvmField
        var GRAFANA_LOGGING: Boolean = false

        @JvmField
        var RDT_DATA_PATH: String? = null

        @JvmField
        var CELEDT_DATA_PATH: String? = null

        @JvmField
        var USDT_DATA_PATH: String? = null

        @JvmField
        var HDT_DATA_PATH: String? = null

        @JvmField
        var GDT_DATA_PATH: String? = null

        @JvmField
        var RSDT_DATA_PATH: String? = null

        @JvmField
        var ASDT_DATA_PATH: String? = null
    }
}
''')

adapter('content/global/activity/ttrail/ClueLevel.java', '''
package content.global.activity.ttrail;

/** The original constant names and order; casket data is outside this oracle. */
public enum ClueLevel {
	EASY,
	MEDIUM,
	HARD,
	UNKNOWN
}
''')

adapter('content/global/activity/ttrail/ClueScrollPlugin.java', '''
package content.global.activity.ttrail;

import core.game.node.item.Item;
import core.tools.Log;
import core.tools.RandomFunction;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import static core.api.ContentAPIKt.log;

/** No clue is registered in this oracle. getClue is the original text. */
public class ClueScrollPlugin {
	public static final Map<ClueLevel, List<ClueScrollPlugin>> ORGANIZED = new HashMap<>();
	private final int clueId;

	public ClueScrollPlugin(int clueId) {
		this.clueId = clueId;
	}

	public int getClueId() {
		return clueId;
	}

	/* ORIGINAL_GET_CLUE */
}
''')

adapter('content/global/handlers/item/equipment/fistofguthixgloves/FOGGlovesManager.kt', '''
package content.global.handlers.item.equipment.fistofguthixgloves

import core.game.node.entity.player.Player

/** Recording service: glove charges are outside this oracle. */
class FOGGlovesManager {
    companion object {
        @JvmStatic
        fun updateCharges(player: Player, charges: Int = 1): Int {
            oracle.Support.emit("fogCharges", player.label)
            return 0
        }
    }
}
''')

adapter('core/game/ge/GrandExchange.java', '''
package core.game.ge;

/** Recording price lookup; prices are case inputs. */
public class GrandExchange {
	public static int getRecommendedPrice(int itemId, boolean from) {
		oracle.Support.emit("price", itemId, from);
		return oracle.Support.price(itemId);
	}
}
''')

adapter('core/game/world/repository/Repository.java', '''
package core.game.world.repository;

import core.game.node.entity.player.Player;
import java.util.List;
import oracle.Support;

/** The case's online players. */
public class Repository {
	public static List<Player> getPlayers() {
		Support.emit("players");
		return Support.repository;
	}

	public static Player getPlayerByUid(int uid) {
		Support.emit("playerByUid", uid);
		return Support.byUid.get(uid);
	}
}
''')

adapter('core/game/node/entity/skill/Skills.java', '''
package core.game.node.entity.skill;

import core.game.node.entity.Entity;
import oracle.Support;

/** Recording skills. PRAYER's value is checked against the original declaration at generation time. */
public class Skills {
	public static final int PRAYER = 5;
	private final Entity entity;

	public Skills(Entity entity) {
		this.entity = entity;
	}

	public int getLifepoints() {
		return entity.lifepoints;
	}

	public void restore() {
		Support.emit("restore", entity.label);
	}

	public void rechargePrayerPoints() {
		Support.emit("rechargePrayer", entity.label);
	}

	public void addExperience(int skill, double experience) {
		Support.emit("xp", entity.label, skill, Double.doubleToRawLongBits(experience) + "");
	}
}
''')
