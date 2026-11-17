"""Adapters for the repository and disconnection queue oracle.

Every class here stands for a system that is not ported. It records the call
and answers from the case's input; `/* ORIGINAL_* */` markers are replaced by
text taken from the pinned sources when the oracle is generated. No adapter
exposes a public field that Kotlin could read in place of a getter the
original calls: the only public fields of the Player adapter are the two the
original declares public (`logoutListeners`, `isAfkLogout`).
"""

ADAPTERS = {}


def adapter(path, text):
    ADAPTERS[path] = text.lstrip('\n')


adapter('core/ServerConstants.java', '''
package core;

/** The three constants Repository.kt reads; the case sets them before Repository is loaded. */
public class ServerConstants {
	public static int MAX_PLAYERS = /* ORIGINAL_MAX_PLAYERS */;
	public static int MAX_NPCS = /* ORIGINAL_MAX_NPCS */;
	public static boolean ENABLE_GLOBAL_CHAT = /* ORIGINAL_ENABLE_GLOBAL_CHAT */;
}
''')

adapter('core/game/node/Node.java', '''
package core.game.node;

/** The index, name and renderable members of Node, as the original has them. */
public class Node {
	public String label;
	protected String name;
	protected int index;
	protected boolean renderable;

	public Node(String name) {
		this.name = name;
	}

	public void oracleName(String name) {
		this.name = name;
	}

	/* ORIGINAL_NODE_GET_NAME */

	/* ORIGINAL_NODE_GET_INDEX */

	/* ORIGINAL_NODE_SET_INDEX */

	/* ORIGINAL_NODE_IS_RENDERABLE */

	/* ORIGINAL_NODE_SET_RENDERABLE */
}
''')

adapter('core/game/node/entity/Entity.java', '''
package core.game.node.entity;

import core.game.interaction.ScriptProcessor;
import core.game.node.Node;
import core.game.node.entity.impl.WalkingQueue;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Location;
import core.game.world.map.zone.ZoneMonitor;
import oracle.Support;

/** Recording entity: every member Player.allowRemoval and Player.finishClear reach. */
public class Entity extends Node {
	public ScriptProcessor scripts = new ScriptProcessor(this);
	private final Skills skills = new Skills(this);
	private final WalkingQueue walkingQueue = new WalkingQueue(this);
	private final ZoneMonitor zoneMonitor = new ZoneMonitor(this);
	protected Location location;

	public Entity(String name) {
		super(name);
	}

	public void oracleLocation(Location location) {
		this.location = location;
	}

	public Location getLocation() {
		return location;
	}

	public Skills getSkills() {
		return skills;
	}

	public WalkingQueue getWalkingQueue() {
		return walkingQueue;
	}

	public ZoneMonitor getZoneMonitor() {
		return zoneMonitor;
	}

	@SuppressWarnings("unchecked")
	public <T> T getAttribute(String key, T fail) {
		Support.emit("attribute", label, key);
		Object value = Support.attribute(this, key);
		return value == null ? fail : (T) value;
	}

	public boolean inCombat() {
		Support.emit("inCombat", label);
		return Support.flag(this, "combat");
	}

	public boolean isTeleporting() {
		Support.emit("teleporting", label);
		return Support.flag(this, "teleporting");
	}

	public void clear() {
		Support.emit("entityClear", label);
	}
}
''')

adapter('core/game/node/entity/player/Player.java', '''
package core.game.node.entity.player;

import content.global.skill.construction.HouseManager;
import content.global.skill.summoning.familiar.FamiliarManager;
import core.game.interaction.QueueStrength;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.DeathTask;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.system.communication.CommunicationInfo;
import core.game.world.GameWorld;
import core.game.world.repository.Repository;
import core.game.world.update.UpdateSequence;
import core.net.IoSession;
import core.worker.ManagementEvents;
import java.util.HashMap;
import kotlin.Unit;
import kotlin.jvm.functions.Function1;
import oracle.Support;
import proto.management.ClanLeaveNotification;
import proto.management.PlayerStatusUpdate;

/**
 * clear, finishClear, sendLogoutEvents, allowRemoval and the accessors below
 * are the original text. checkForWealthUpdate records its call; the original
 * body (wealth totals, monitor log) is not exercised.
 */
public class Player extends Entity {
	/* ORIGINAL_PLAYER_DETAILS_FIELD */

	/* ORIGINAL_PLAYER_LOGOUT_LISTENERS_FIELD */

	/* ORIGINAL_PLAYER_AFK_FIELD */

	/* ORIGINAL_PLAYER_PACKET_DISPATCH_FIELD */

	/* ORIGINAL_PLAYER_INTERFACE_MANAGER_FIELD */

	/* ORIGINAL_PLAYER_FAMILIAR_MANAGER_FIELD */

	/* ORIGINAL_PLAYER_PLAYING_FIELD */

	/* ORIGINAL_PLAYER_ARTIFICIAL_FIELD */

	public Player(PlayerDetails details) {
		super(details.getUsername());
		this.details = details;
	}

	public void oracleArtificial(boolean artificial) {
		this.artificial = artificial;
	}

	/* ORIGINAL_PLAYER_CLEAR */

	/* ORIGINAL_PLAYER_FINISH_CLEAR */

	/* ORIGINAL_PLAYER_SEND_LOGOUT_EVENTS */

	private void checkForWealthUpdate(boolean force) {
		Support.emit("wealthUpdate", label, force);
	}

	/* ORIGINAL_PLAYER_ALLOW_REMOVAL */

	/* ORIGINAL_PLAYER_GET_DETAILS */

	/* ORIGINAL_PLAYER_GET_NAME */

	/* ORIGINAL_PLAYER_GET_SESSION */

	/* ORIGINAL_PLAYER_SET_PLAYING */

	/* ORIGINAL_PLAYER_IS_PLAYING */

	/* ORIGINAL_PLAYER_GET_PACKET_DISPATCH */

	/* ORIGINAL_PLAYER_IS_ARTIFICIAL */

	/* ORIGINAL_PLAYER_GET_COMMUNICATION */
}
''')

adapter('core/game/node/entity/player/info/PlayerDetails.java', '''
package core.game.node.entity.player.info;

import core.game.system.communication.CommunicationInfo;
import core.net.IoSession;
import oracle.Support;

/** Holds the values the case gives; save() records its call. */
public class PlayerDetails {
	public String label;
	private String usernameValue;
	private int uidValue;
	private IoSession sessionValue;
	private final CommunicationInfo communicationInfo = new CommunicationInfo(this);

	public PlayerDetails(String username) {
		this.usernameValue = username;
	}

	public void oracleUsername(String username) {
		this.usernameValue = username;
	}

	public void oracleUid(int uid) {
		this.uidValue = uid;
	}

	public String getUsername() {
		return usernameValue;
	}

	public int getUid() {
		return uidValue;
	}

	public IoSession getSession() {
		return sessionValue;
	}

	public void setSession(IoSession session) {
		this.sessionValue = session;
	}

	public CommunicationInfo getCommunication() {
		return communicationInfo;
	}

	public void save() {
		Support.emit("detailsSave", label);
	}
}
''')

adapter('core/game/node/entity/player/info/login/PlayerParser.java', '''
package core.game.node.entity.player.info.login;

import core.game.node.entity.player.Player;
import oracle.Support;

/** The save itself is the player-save port's; here it is a recorded hand-off. */
public final class PlayerParser {
	public static void saveImmediately(Player player) {
		Support.emit("saveImmediately", player.label, Thread.currentThread().getName());
	}
}
''')

adapter('core/game/node/entity/player/link/PacketDispatch.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;
import core.game.world.repository.Repository;
import core.net.packet.PacketRepository;
import core.net.packet.context.PlayerContext;
import core.net.packet.out.Logout;

/** sendLogout is the original text; the packet is a recorded call. */
public final class PacketDispatch {
	private final Player player;
	private final PlayerContext context;

	public PacketDispatch(Player player) {
		this.player = player;
		this.context = new PlayerContext(player);
	}

	/* ORIGINAL_DISPATCH_SEND_LOGOUT */

	public PlayerContext getContext() {
		return context;
	}
}
''')

adapter('core/net/packet/PacketRepository.java', '''
package core.net.packet;

import core.net.packet.context.PlayerContext;
import oracle.Support;

public final class PacketRepository {
	public static void send(Class<?> packet, PlayerContext context) {
		Support.emit("logoutPacket", context.getPlayer().label);
	}
}
''')

adapter('core/net/packet/context/PlayerContext.java', '''
package core.net.packet.context;

import core.game.node.entity.player.Player;

public final class PlayerContext {
	private final Player player;

	public PlayerContext(Player player) {
		this.player = player;
	}

	public Player getPlayer() {
		return player;
	}
}
''')

adapter('core/net/packet/out/Logout.java', '''
package core.net.packet.out;

public final class Logout {
}
''')

adapter('core/game/node/entity/player/link/InterfaceManager.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;
import oracle.Support;

public final class InterfaceManager {
	private final Player player;

	public InterfaceManager(Player player) {
		this.player = player;
	}

	public boolean close() {
		Support.emit("interfaceClose", player.label);
		return true;
	}

	public boolean closeSingleTab() {
		Support.emit("closeSingleTab", player.label);
		return true;
	}
}
''')

adapter('content/global/skill/summoning/familiar/FamiliarManager.java', '''
package content.global.skill.summoning.familiar;

import core.game.node.entity.player.Player;
import oracle.Support;

public final class FamiliarManager {
	private final Player player;
	private final Familiar familiar;

	public FamiliarManager(Player player) {
		this.player = player;
		this.familiar = new Familiar(player);
	}

	public boolean hasFamiliar() {
		Support.emit("hasFamiliar", player.label);
		return Support.flag(player, "familiar");
	}

	public Familiar getFamiliar() {
		return familiar;
	}

	public static final class Familiar {
		private final Player owner;

		Familiar(Player owner) {
			this.owner = owner;
		}

		public void clear() {
			Support.emit("familiarClear", owner.label);
		}
	}
}
''')

adapter('content/global/skill/construction/HouseManager.java', '''
package content.global.skill.construction;

import core.game.node.entity.player.Player;
import oracle.Support;

public final class HouseManager {
	public static void leave(Player player) {
		Support.emit("houseLeave", player.label);
	}
}
''')

adapter('core/game/system/communication/CommunicationInfo.java', '''
package core.game.system.communication;

import core.game.node.entity.player.info.PlayerDetails;
import oracle.Support;

public final class CommunicationInfo {
	private final PlayerDetails details;

	public CommunicationInfo(PlayerDetails details) {
		this.details = details;
	}

	public ClanRepository getClan() {
		Support.emit("clan", details.label);
		return Support.clan(details.label);
	}
}
''')

adapter('core/game/system/communication/ClanRepository.java', '''
package core.game.system.communication;

import core.game.node.entity.player.Player;
import oracle.Support;

public final class ClanRepository {
	private final String ownerValue;

	public ClanRepository(String owner) {
		this.ownerValue = owner;
	}

	public String getOwner() {
		Support.emit("clanOwner", ownerValue);
		return ownerValue;
	}

	public void leave(Player player, boolean remove) {
		Support.emit("clanLeave", ownerValue, player.label, remove);
	}
}
''')

adapter('core/game/world/map/zone/ZoneMonitor.java', '''
package core.game.world.map.zone;

import core.game.node.entity.Entity;
import oracle.Support;

public final class ZoneMonitor {
	private final Entity entity;

	public ZoneMonitor(Entity entity) {
		this.entity = entity;
	}

	public boolean clear() {
		Support.emit("zoneClear", entity.label);
		return true;
	}
}
''')

adapter('core/game/node/entity/impl/WalkingQueue.java', '''
package core.game.node.entity.impl;

import core.game.node.entity.Entity;
import oracle.Support;

public final class WalkingQueue {
	private final Entity entity;

	public WalkingQueue(Entity entity) {
		this.entity = entity;
	}

	public void reset() {
		Support.emit("walkReset", entity.label);
	}
}
''')

adapter('core/game/node/entity/skill/Skills.java', '''
package core.game.node.entity.skill;

import core.game.node.entity.Entity;
import oracle.Support;

public final class Skills {
	private final Entity entity;

	public Skills(Entity entity) {
		this.entity = entity;
	}

	public int getLifepoints() {
		Support.emit("lifepoints", entity.label);
		return Support.number(entity, "lifepoints", 10);
	}
}
''')

adapter('core/game/node/entity/combat/DeathTask.java', '''
package core.game.node.entity.combat;

import core.game.node.entity.Entity;
import oracle.Support;

public final class DeathTask {
	public static boolean isDead(Entity entity) {
		Support.emit("isDead", entity.label);
		return Support.flag(entity, "dead");
	}
}
''')

adapter('core/game/interaction/QueueStrength.java', '''
package core.game.interaction;

/** The four constants of the original enum (Script.kt), in its order. */
public enum QueueStrength {
	/* ORIGINAL_QUEUE_STRENGTH */
}
''')

adapter('core/game/interaction/ScriptProcessor.java', '''
package core.game.interaction;

import core.game.node.entity.Entity;
import oracle.Support;

public final class ScriptProcessor {
	private final Entity entity;

	public ScriptProcessor(Entity entity) {
		this.entity = entity;
	}

	public boolean hasTypeInQueue(QueueStrength type) {
		Support.emit("scriptQueue", entity.label, type.name());
		return Support.flag(entity, "soft");
	}
}
''')

adapter('proto/management/PlayerStatusUpdate.java', '''
package proto.management;

/** Field holder for the generated message. A null string is refused as the generated builder refuses it. */
public final class PlayerStatusUpdate {
	public final String username;
	public final int world;
	public final boolean notifyFriendsOnly;

	private PlayerStatusUpdate(Builder builder) {
		username = builder.username;
		world = builder.world;
		notifyFriendsOnly = builder.notifyFriendsOnly;
	}

	public static Builder newBuilder() {
		return new Builder();
	}

	public static final class Builder {
		private String username = "";
		private int world;
		private boolean notifyFriendsOnly;

		public Builder setUsername(String value) {
			if (value == null) {
				throw new NullPointerException();
			}
			username = value;
			return this;
		}

		public Builder setWorld(int value) {
			world = value;
			return this;
		}

		public Builder setNotifyFriendsOnly(boolean value) {
			notifyFriendsOnly = value;
			return this;
		}

		public PlayerStatusUpdate build() {
			return new PlayerStatusUpdate(this);
		}
	}
}
''')

adapter('proto/management/ClanLeaveNotification.java', '''
package proto.management;

/** Field holder for the generated message. A null string is refused as the generated builder refuses it. */
public final class ClanLeaveNotification {
	public final String username;
	public final int world;
	public final String clanName;

	private ClanLeaveNotification(Builder builder) {
		username = builder.username;
		world = builder.world;
		clanName = builder.clanName;
	}

	public static Builder newBuilder() {
		return new Builder();
	}

	public static final class Builder {
		private String username = "";
		private int world;
		private String clanName = "";

		public Builder setUsername(String value) {
			if (value == null) {
				throw new NullPointerException();
			}
			username = value;
			return this;
		}

		public Builder setWorld(int value) {
			world = value;
			return this;
		}

		public Builder setClanName(String value) {
			if (value == null) {
				throw new NullPointerException();
			}
			clanName = value;
			return this;
		}

		public ClanLeaveNotification build() {
			return new ClanLeaveNotification(this);
		}
	}
}
''')

adapter('core/worker/ManagementEvents.java', '''
package core.worker;

import oracle.Support;
import proto.management.ClanLeaveNotification;
import proto.management.PlayerStatusUpdate;

public final class ManagementEvents {
	public static void publish(Object message) {
		if (message instanceof PlayerStatusUpdate) {
			PlayerStatusUpdate update = (PlayerStatusUpdate) message;
			Support.emit("publishStatus", update.username, update.world, update.notifyFriendsOnly);
		} else {
			ClanLeaveNotification event = (ClanLeaveNotification) message;
			Support.emit("publishClanLeave", event.username, event.world, event.clanName);
		}
	}
}
''')

adapter('core/game/world/GameWorld.kt', '''
package core.game.world

import core.api.LogoutListener

/** The three members the queue and Player.finishClear read. */
object GameWorld {
    @JvmStatic
    var ticks: Int
        get() = oracle.Support.ticks
        set(value) { oracle.Support.ticks = value }

    @JvmStatic
    val logoutListeners = ArrayList<LogoutListener>()

    @JvmStatic
    val settings: GameSettings?
        get() = oracle.Support.settings()
}

class GameSettings(private val world: Int) {
    val worldId: Int
        get() {
            oracle.Support.emit("worldId")
            return world
        }
}
''')

adapter('core/game/world/update/UpdateSequence.kt', '''
package core.game.world.update

/** Only the renderable player list; its removal is a recorded call. */
class UpdateSequence {
    companion object {
        @JvmStatic
        val renderablePlayers = oracle.Support.RenderList()
    }
}
''')

adapter('core/api/Recorded.kt', '''
package core.api

import core.game.node.entity.player.Player
import core.tools.Log

fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", origin.simpleName, type.name, message)
}

fun sendMessage(player: Player, message: String) {
    oracle.Support.emit("message", player.label, message)
}
''')

adapter('core/tools/Log.kt', '''
package core.tools

/* ORIGINAL_LOG_ENUM */
''')

adapter('core/net/IoSession.java', '''
package core.net;

import core.game.node.entity.player.Player;
import core.game.world.repository.Repository;
import java.io.IOException;
import java.nio.channels.SelectionKey;
import java.nio.channels.SocketChannel;

/** disconnect, getPlayer, isActive and setObject are the original text. */
public class IoSession {
	public String label;
	/* ORIGINAL_SESSION_KEY_FIELD */
	/* ORIGINAL_SESSION_OBJECT_FIELD */
	/* ORIGINAL_SESSION_ACTIVE_FIELD */
	private String associatedUsername;

	public IoSession(SelectionKey key, String associatedUsername) {
		this.key = key;
		this.associatedUsername = associatedUsername;
	}

	public void oracleActive(boolean active) {
		this.active = active;
	}

	public Object oracleObject() {
		return object;
	}

	/* ORIGINAL_SESSION_DISCONNECT */

	/* ORIGINAL_SESSION_IS_ACTIVE */

	/* ORIGINAL_SESSION_GET_PLAYER */

	/* ORIGINAL_SESSION_SET_OBJECT */
}
''')

adapter('core/game/node/entity/npc/NPC.java', '''
package core.game.node.entity.npc;

import core.game.node.entity.Entity;

/** getId and its field are the original text. */
public class NPC extends Entity {
	/* ORIGINAL_NPC_ID_FIELD */

	public NPC(String name) {
		super(name);
	}

	public void oracleId(int id) {
		this.id = id;
	}

	/* ORIGINAL_NPC_GET_ID */
}
''')

adapter('content/region/wilderness/handlers/revenants/RevenantNPC.java', '''
package content.region.wilderness.handlers.revenants;

import core.game.node.entity.npc.NPC;

public class RevenantNPC extends NPC {
	public RevenantNPC(String name) {
		super(name);
	}
}
''')

adapter('core/game/world/map/Location.java', '''
package core.game.world.map;

/** Three coordinates and equality on them; the original class is not compiled. */
public final class Location {
	public final int x;
	public final int y;
	public final int z;

	public Location(int x, int y, int z) {
		this.x = x;
		this.y = y;
		this.z = z;
	}

	@Override
	public boolean equals(Object other) {
		if (!(other instanceof Location)) {
			return false;
		}
		Location location = (Location) other;
		return location.x == x && location.y == y && location.z == z;
	}

	@Override
	public int hashCode() {
		return z << 30 | x << 15 | y;
	}
}
''')

adapter('core/game/world/map/RegionManager.java', '''
package core.game.world.map;

import oracle.Support;

public final class RegionManager {
	public static RegionChunk getRegionChunk(Location location) {
		Support.emit("regionChunk", location.x, location.y, location.z);
		return new RegionChunk();
	}
}
''')

adapter('core/game/world/map/RegionChunk.java', '''
package core.game.world.map;

import core.game.node.entity.npc.NPC;
import java.util.List;
import oracle.Support;

public final class RegionChunk {
	public List<NPC> getNpcs() {
		return Support.chunkNpcs();
	}
}
''')

adapter('kotlinx/coroutines/Coroutines.kt', '''
// Oracle-only stand-in for the two kotlinx.coroutines symbols TaskExecutor.kt
// imports. The pinned compiler artifacts do not include kotlinx-coroutines.
// A launched block is recorded and kept; the case runs it later on the calling
// thread. Dispatcher threads and real scheduling are not part of this corpus.
package kotlinx.coroutines

import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine

interface CoroutineScope

object GlobalScope : CoroutineScope

fun CoroutineScope.launch(block: suspend CoroutineScope.() -> Unit) {
    val scope = this
    oracle.Support.launch(Runnable {
        var outcome: Result<Unit>? = null
        block.startCoroutine(scope, Continuation(EmptyCoroutineContext) { outcome = it })
        val finished = outcome ?: throw IllegalStateException("oracle coroutine adapter cannot suspend")
        finished.getOrThrow()
    })
}
''')
