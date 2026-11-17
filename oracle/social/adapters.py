"""Adapters for the social oracle.

Every class here stands for a system that is not ported by the social slice.
It records the call and answers from the case's input; `/* ORIGINAL_* */`
markers are replaced by text taken from the pinned sources when the oracle is
generated. No adapter exposes a public field that Kotlin could read in place
of a getter the original calls: the only public fields are the ones the
original declares public (`PlayerDetails.accountInfo`).
"""

ADAPTERS = {}


def adapter(path, text):
    ADAPTERS[path] = text.lstrip('\n')


adapter('core/ServerConstants.java', '''
package core;

/** The three constants the selected code reads, set from the case before this class loads. */
public class ServerConstants {
	public static String SERVER_NAME = oracle.Support.text("serverName");
	public static String MS_SECRET_KEY = oracle.Support.text("msKey");
	public static boolean ENABLE_GLOBAL_CHAT = oracle.Support.flag("globalChat");
}
''')

adapter('core/game/system/mysql/SQLTable.java', '''
package core.game.system.mysql;

/** The legacy column path of CommunicationInfo is verified by the player oracle; no case here reaches it. */
public class SQLTable {
	public SQLColumn getColumn(String name) {
		throw new UnsupportedOperationException("not part of the social oracle");
	}
}
''')

adapter('core/game/system/mysql/SQLColumn.java', '''
package core.game.system.mysql;

/** See SQLTable. */
public class SQLColumn {
	public Object getValue() {
		throw new UnsupportedOperationException("not part of the social oracle");
	}

	public void updateValue(Object value) {
		throw new UnsupportedOperationException("not part of the social oracle");
	}
}
''')

adapter('core/game/node/Node.java', '''
package core.game.node;

/** Node.getName as the original has it. */
public class Node {
	protected String name;

	public Node(String name) {
		this.name = name;
	}

	/* ORIGINAL_NODE_GET_NAME */
}
''')

adapter('core/game/node/entity/Entity.java', '''
package core.game.node.entity;

import core.game.node.Node;
import core.game.node.entity.lock.ActionLocks;
import core.game.world.update.UpdateMasks;

/** Attributes, locks and masks are other systems: every access is a recorded call. */
public class Entity extends Node {
	private final ActionLocks locks = new ActionLocks(this);
	private final UpdateMasks updateMasks = new UpdateMasks(this);

	public Entity(String name) {
		super(name);
	}

	@SuppressWarnings("unchecked")
	public <T> T getAttribute(String key, T fail) {
		return (T) oracle.Support.getAttribute(this, key, fail);
	}

	@SuppressWarnings("unchecked")
	public <T> T getAttribute(String key) {
		return (T) oracle.Support.getAttribute(this, key, null);
	}

	public void setAttribute(String key, Object value) {
		oracle.Support.setAttribute(this, key, value);
	}

	public void removeAttribute(String key) {
		oracle.Support.removeAttribute(this, key);
	}

	public ActionLocks getLocks() {
		return locks;
	}

	public UpdateMasks getUpdateMasks() {
		return updateMasks;
	}
}
''')

adapter('core/game/node/entity/lock/ActionLocks.java', '''
package core.game.node.entity.lock;

import core.game.node.entity.Entity;

/** The lock system is not ported here: both calls are recorded and answered by the case. */
public final class ActionLocks {
	private final Entity owner;

	public ActionLocks(Entity owner) {
		this.owner = owner;
	}

	public boolean isLocked(String key) {
		return oracle.Support.isLocked(owner, key);
	}

	public void lock(String key, int ticks) {
		oracle.Support.lock(owner, key, ticks);
	}
}
''')

adapter('core/game/world/update/UpdateMasks.kt', '''
package core.game.world.update

import core.game.node.entity.Entity
import core.game.world.update.flag.EntityFlag

/** The mask registry is ported elsewhere (world/update_masks): this records the registration. */
class UpdateMasks(private val owner: Entity) {
    fun register(flag: EntityFlag, context: Any?, sync: Boolean = false): Boolean {
        return oracle.Support.registerMask(owner, flag.name, context, sync)
    }
}
''')

adapter('core/game/world/update/flag/EntityFlag.kt', '''
package core.game.world.update.flag

/** The one constant the selected code names. */
enum class EntityFlag {
    Chat
}
''')

adapter('core/game/node/entity/player/Player.java', '''
package core.game.node.entity.player;

import core.game.node.entity.Entity;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.link.InterfaceManager;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.Settings;
import core.game.system.communication.CommunicationInfo;
import core.game.world.GameWorld;
import core.net.IoSession;
import core.tools.StringUtils;
import core.worker.ManagementEvents;
import proto.management.ClanLeaveNotification;
import proto.management.PlayerStatusUpdate;

/** The Player members the selected code calls; the original text is inserted where it is used. */
public class Player extends Entity {
	private final String oracleLabel;
	private final PlayerDetails details;
	private final Settings settings = new Settings(this);
	private final PacketDispatch packetDispatch = new PacketDispatch(this);
	private final InterfaceManager interfaceManager = new InterfaceManager(this);
	private final IoSession session;
	protected boolean artificial;

	public Player(String label, String name, PlayerDetails details, boolean session, boolean artificial) {
		super(name);
		this.oracleLabel = label;
		this.details = details;
		this.session = session ? new IoSession(this) : null;
		this.artificial = artificial;
	}

	public String oracleLabel() {
		return oracleLabel;
	}

	public PlayerDetails getDetails() {
		return details;
	}

	public Settings getSettings() {
		return settings;
	}

	public IoSession getSession() {
		return session;
	}

	public PacketDispatch getPacketDispatch() {
		return packetDispatch;
	}

	public InterfaceManager getInterfaceManager() {
		return interfaceManager;
	}

	public boolean isArtificial() {
		return artificial;
	}

	public void sendMessage(String message) {
		oracle.Support.call("playerMessage", oracle.Support.label(this), message);
	}

	public void oracleSendLogoutEvents() {
		sendLogoutEvents();
	}

	/* ORIGINAL_PLAYER_GET_NAME */

	/* ORIGINAL_PLAYER_GET_COMMUNICATION */

	/* ORIGINAL_PLAYER_GET_USERNAME */

	/* ORIGINAL_PLAYER_SEND_LOGOUT_EVENTS */
}
''')

adapter('core/game/bots/AIPlayer.java', '''
package core.game.bots;

import core.game.node.entity.player.Player;
import core.game.node.entity.player.info.PlayerDetails;
import core.net.packet.context.MessageContext;

/** A bot: only the incoming-chat hook CommunicationMessage calls. */
public class AIPlayer extends Player {
	public AIPlayer(String label, String name, PlayerDetails details, boolean session) {
		super(label, name, details, session, true);
	}

	public void handleIncomingChat(MessageContext context) {
		oracle.Support.call("aiChat", oracle.Support.label(this), context.getOpcode(), context.getOther(), context.getMessage());
	}
}
''')

adapter('core/game/node/entity/player/info/PlayerDetails.java', '''
package core.game.node.entity.player.info;

import core.auth.UserAccountInfo;
import core.game.system.communication.CommunicationInfo;
import org.jetbrains.annotations.NotNull;

/** The PlayerDetails members the selected code calls, with the original text of each. */
public class PlayerDetails {
	/* ORIGINAL_DETAILS_ACCOUNT_FIELD */

	/* ORIGINAL_DETAILS_COMMUNICATION_FIELD */

	/* ORIGINAL_DETAILS_CONSTRUCTOR */

	/* ORIGINAL_DETAILS_IS_MUTED */

	/* ORIGINAL_DETAILS_GET_RIGHTS */

	/* ORIGINAL_DETAILS_GET_COMMUNICATION */

	/* ORIGINAL_DETAILS_GET_USERNAME */

	/* ORIGINAL_DETAILS_GET_DETAILS */
}
''')

adapter('core/game/node/entity/player/link/Settings.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;
import core.net.packet.IoBuffer;

/** The three chat fields and the chat-setting methods of Settings, as the original has them. */
public final class Settings {
	/* ORIGINAL_SETTINGS_PLAYER_FIELD */

	/* ORIGINAL_SETTINGS_PUBLIC_FIELD */

	/* ORIGINAL_SETTINGS_PRIVATE_FIELD */

	/* ORIGINAL_SETTINGS_TRADE_FIELD */

	public Settings(Player player) {
		this.player = player;
	}

	public void oracleInitial(int publicSetting, int privateSetting, int tradeSetting) {
		publicChatSetting = publicSetting;
		privateChatSetting = privateSetting;
		this.tradeSetting = tradeSetting;
	}

	/* ORIGINAL_SETTINGS_UPDATE_CHAT */

	/* ORIGINAL_SETTINGS_UPDATE_CHAT_VALUES */

	/* ORIGINAL_SETTINGS_GET_PUBLIC */

	/* ORIGINAL_SETTINGS_GET_PRIVATE */

	/* ORIGINAL_SETTINGS_GET_TRADE */
}
''')

adapter('core/game/node/entity/player/link/PacketDispatch.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;

/** Game messages and interface strings are other systems: recorded calls. */
public final class PacketDispatch {
	private final Player player;

	public PacketDispatch(Player player) {
		this.player = player;
	}

	public void sendMessage(String message) {
		oracle.Support.call("dispatchMessage", oracle.Support.label(player), message);
	}

	public void sendString(String string, int interfaceId, int lineId) {
		oracle.Support.call("sendString", oracle.Support.label(player), string, interfaceId, lineId);
	}
}
''')

adapter('core/game/node/entity/player/link/InterfaceManager.java', '''
package core.game.node.entity.player.link;

import core.game.component.Component;
import core.game.node.entity.player.Player;

/** Opening a component is recorded; the window mode is the case's. */
public final class InterfaceManager {
	private final Player player;

	public InterfaceManager(Player player) {
		this.player = player;
	}

	public Component open(Component component) {
		oracle.Support.call("open", oracle.Support.label(player), component.getId());
		return component;
	}

	public boolean isResizable() {
		return oracle.Support.resizable(player);
	}
}
''')

adapter('core/game/component/Component.java', '''
package core.game.component;

/** A component as far as ClanRepository.openSettings constructs one. */
public class Component {
	private final int id;

	public Component(int id) {
		this.id = id;
	}

	public int getId() {
		return id;
	}
}
''')

adapter('core/net/IoSession.java', '''
package core.net;

import core.cache.crypto.ISAACPair;
import core.game.node.entity.player.Player;

/** A session: writes and the outgoing cipher are recorded. A null owner is the management server's session. */
public class IoSession {
	private final Player owner;
	private final ISAACPair isaacPair;

	public IoSession(Player owner) {
		this.owner = owner;
		this.isaacPair = new ISAACPair(owner);
	}

	public ISAACPair getIsaacPair() {
		return isaacPair;
	}

	public void write(Object context) {
		oracle.Support.write(owner, context);
	}
}
''')

adapter('core/cache/crypto/ISAACPair.java', '''
package core.cache.crypto;

import core.game.node.entity.player.Player;

/** Only the output cipher is read by the selected code. */
public final class ISAACPair {
	private final ISAACCipher output;

	public ISAACPair(Player owner) {
		this.output = new ISAACCipher(owner);
	}

	public ISAACCipher getOutput() {
		return output;
	}
}
''')

adapter('core/cache/crypto/ISAACCipher.java', '''
package core.cache.crypto;

import core.game.node.entity.player.Player;

/** The opcode cipher is a session input: each draw is recorded and answered by the case. */
public final class ISAACCipher {
	private final Player owner;

	public ISAACCipher(Player owner) {
		this.owner = owner;
	}

	public int getNextValue() {
		return oracle.Support.cipher(owner);
	}
}
''')

adapter('core/cache/misc/buffer/ByteBufferUtils.java', '''
package core.cache.misc.buffer;

import java.nio.ByteBuffer;

/** IoBuffer's string readers/writers that the selected code never reaches. */
public final class ByteBufferUtils {
	public static String getString(ByteBuffer buffer) {
		throw new UnsupportedOperationException("not part of the social oracle");
	}

	public static int packGJString2(int position, byte[] buffer, String string) {
		throw new UnsupportedOperationException("not part of the social oracle");
	}
}
''')

adapter('core/game/world/GameWorld.kt', '''
package core.game.world

import core.auth.AuthProvider
import core.storage.AccountStorageProvider

/** The world members the selected code reads, answered by the case. */
object GameWorld {
    @JvmStatic
    var settings: GameSettings? = oracle.Support.gameSettings()
    @JvmStatic
    val authenticator: AuthProvider<*>
        get() = oracle.Support.authenticator()
    @JvmStatic
    val accountStorage: AccountStorageProvider
        get() = oracle.Support.accountStorage()
    @JvmStatic
    var Pulser = PulseRunner()
}
''')

adapter('core/game/world/GameSettings.kt', '''
package core.game.world

/** The four settings the selected code reads. */
class GameSettings(name: String, isDevMode: Boolean, worldId: Int, enable_default_clan: Boolean) {
    var name: String = name
    var isDevMode: Boolean = isDevMode
    var worldId: Int = worldId
    var enable_default_clan: Boolean = enable_default_clan
}
''')

adapter('core/game/world/PulseRunner.kt', '''
package core.game.world

import core.game.system.task.Pulse

/** The world pulse queue is ported elsewhere (pulses): a submission is recorded and kept for the case. */
class PulseRunner {
    fun submit(pulse: Pulse) {
        oracle.Support.submit(pulse)
    }
}
''')

adapter('core/game/system/task/Pulse.java', '''
package core.game.system.task;

import core.game.node.Node;

/** A pulse as the loot-share toggle builds one; the case decides when it runs. */
public abstract class Pulse {
	private final int delay;

	public Pulse(int delay, Node... checks) {
		this.delay = delay;
	}

	public int oracleDelay() {
		return delay;
	}

	public abstract boolean pulse();

	public void stop() {
		oracle.Support.stopPulse(this);
	}
}
''')

adapter('core/storage/AccountStorageProvider.kt', '''
package core.storage

import core.auth.UserAccountInfo

/** The three account-storage calls ManagementEvents makes, recorded and answered by the case. */
interface AccountStorageProvider {
    fun getAccountInfo(username: String): UserAccountInfo
    fun getOnlineFriends(username: String): List<String>
    fun update(info: UserAccountInfo)
}
''')

adapter('core/auth/AuthProvider.kt', '''
package core.auth

/** The one authenticator call ManagementEvents makes. */
abstract class AuthProvider<T> {
    abstract fun createAccountWith(info: UserAccountInfo): Boolean
}
''')

adapter('core/api/ContentAPI.kt', '''
@file:JvmName("ContentAPIKt")
package core.api

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.tools.Log

/** Varps, logging and game messages are other systems: recorded calls. */
@JvmOverloads fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) {
    oracle.Support.call("varp", oracle.Support.label(player), varpIndex, value, save)
}

fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.log(origin, type.name, message)
}

fun sendMessage(player: Player, message: String) {
    oracle.Support.call("apiMessage", oracle.Support.label(player), message)
}

/* ORIGINAL_API_GET_ATTRIBUTE */

/* ORIGINAL_API_SET_ATTRIBUTE */
''')

adapter('core/api/Commands.kt', '''
package core.api

import core.game.node.entity.player.Player
import core.game.system.command.Privilege

/** Command registration is another system: define records the command and keeps its handler for the case. */
interface Commands : ContentInterface {
    fun define(name: String, privilege: Privilege = Privilege.ADMIN, usage: String = "", description: String = "", handle: (Player, Array<String>) -> Unit) {
        oracle.Support.define(name, privilege.name, usage, description, handle)
    }

    fun defineCommands()
}
''')

adapter('core/tools/SystemLogger.kt', '''
package core.tools

/** The management log line is recorded. */
object SystemLogger {
    @JvmStatic
    fun logMS(s: String) {
        oracle.Support.call("logMS", s)
    }
}

/* ORIGINAL_LOG_ENUM */
''')

adapter('core/game/node/entity/player/info/PlayerMonitor.kt', '''
package core.game.node.entity.player.info

import core.game.node.entity.player.Player

/** Chat logging is a moderation tool outside this slice: recorded calls. */
object PlayerMonitor {
    @JvmStatic
    fun logChat(player: Player, type: String, message: String) {
        oracle.Support.call("logChat", oracle.Support.label(player), type, message)
    }

    @JvmStatic
    fun logPrivateChat(sender: Player, receiver: String, message: String) {
        oracle.Support.call("logPrivateChat", oracle.Support.label(sender), receiver, message)
    }
}
''')

adapter('core/game/activity/ActivityPlugin.java', '''
package core.game.activity;

/** Clan wars: no case sets one, so ClanRepository.leave never reaches this. */
public abstract class ActivityPlugin {
	public Object fireEvent(String identifier, Object... args) {
		oracle.Support.call("fireEvent", identifier);
		return null;
	}
}
''')

adapter('core/net/amsc/WorldCommunicator.java', '''
package core.net.amsc;

import core.net.IoSession;

/** The management-server link. The pinned Server.kt never connects; the case may choose otherwise. */
public final class WorldCommunicator {
	/* ORIGINAL_WORLD_COMMUNICATOR_STATE_FIELD */

	private static IoSession session = oracle.Support.managementSession();

	static {
		state = oracle.Support.managementState();
	}

	public static IoSession getSession() {
		return session;
	}

	/* ORIGINAL_WORLD_COMMUNICATOR_IS_ENABLED */
}
''')

adapter('core/net/packet/PacketRepository.java', '''
package core.net.packet;

import core.net.packet.out.*;
import core.tools.Log;

import java.io.PrintWriter;
import java.io.StringWriter;
import java.util.HashMap;
import java.util.Map;
import core.game.bots.AIPlayer;

import static core.api.ContentAPIKt.log;

/** The original send, over the three outgoing packets of this slice. */
public final class PacketRepository {
	public final static Map<Class<?>, OutgoingPacket<? extends Context>> OUTGOING_PACKETS = new HashMap<>();

	static {
		OUTGOING_PACKETS.put(ContactPackets.class, new ContactPackets());
		OUTGOING_PACKETS.put(CommunicationMessage.class, new CommunicationMessage());
		OUTGOING_PACKETS.put(UpdateClanChat.class, new UpdateClanChat());
	}

	/* ORIGINAL_PACKET_REPOSITORY_SEND */
}
''')

for name in ['UpdateSceneGraph', 'BuildDynamicScene', 'InstancedLocationUpdate', 'Logout', 'ClearRegionChunk']:
    adapter(f'core/net/packet/out/{name}.java', f'''
package core.net.packet.out;

import core.net.packet.Context;
import core.net.packet.OutgoingPacket;

/** Named by PacketWriteQueue.handle; never sent by this corpus. */
public final class {name} implements OutgoingPacket<Context> {{
	@Override
	public void send(Context context) {{
		throw new UnsupportedOperationException("not part of the social oracle");
	}}
}}
''')

adapter('core/net/packet/in/QCPacketType.kt', '''
package core.net.packet.`in`

/** Named by Packet.QuickChat; quick chat is not part of this slice. */
enum class QCPacketType {
    STANDARD
}
''')

adapter('core/game/world/repository/Repository.kt', '''
package core.game.world.repository

import core.game.node.entity.player.Player

/** The player list and name map (ported in world/repository) are case inputs; the name lookup is the original. */
object Repository {
    @JvmStatic
    val players: MutableList<Player> = ArrayList()

    @JvmStatic
    val playerNames: MutableMap<String, Player> = HashMap()

    @JvmStatic
    /* ORIGINAL_REPOSITORY_GET_PLAYER_BY_NAME */
}
''')

adapter('core/net/packet/PacketProcessor.kt', '''
package core.net.packet

import core.ServerConstants
import core.api.getAttribute
import core.api.sendMessage
import core.game.node.entity.player.info.PlayerMonitor
import core.game.node.entity.player.info.Rights
import core.game.system.communication.ClanRank
import core.game.system.communication.CommunicationInfo
import core.game.system.communication.GlobalChat
import core.game.world.repository.Repository
import core.game.world.update.flag.EntityFlag
import core.game.world.update.flag.context.ChatMessage
import core.net.amsc.MSPacketRepository
import core.net.packet.`in`.Packet
import core.worker.ManagementEvents
import proto.management.ClanMessage
import proto.management.JoinClanRequest
import proto.management.LeaveClanRequest
import java.lang.Math.min

/** The social branches of PacketProcessor.process and splitChatMessage, as the original has them. */
object PacketProcessor {
    @JvmStatic
    fun oracleProcess(pkt: Packet) {
        process(pkt)
    }

    private fun process(pkt: Packet) {
        when (pkt) {
/* ORIGINAL_PROCESS_FRIENDS */
/* ORIGINAL_PROCESS_CLANS */
/* ORIGINAL_PROCESS_CHAT */
            else -> oracle.Support.unprocessed(pkt)
        }
    }

/* ORIGINAL_SPLIT_CHAT_MESSAGE */

/* ORIGINAL_BASE_CHAT_CUTOFF */
}
''')

adapter('core/net/packet/in/Login.kt', '''
package core.net.packet.`in`

import core.game.node.entity.player.info.PlayerDetails
import core.game.world.GameWorld
import core.worker.ManagementEvents.publish
import proto.management.JoinClanRequest
import proto.management.PlayerStatusUpdate
import proto.management.RequestContactInfo

/** Login.sendMSEvents, as the original has it. */
object Login {
    @JvmStatic
    fun oracleSendMSEvents(details: PlayerDetails) {
        sendMSEvents(details)
    }

/* ORIGINAL_LOGIN_SEND_MS_EVENTS */
}
''')

adapter('core/net/packet/in/Decoders530.kt', '''
package core.net.packet.`in`

import core.Util.clamp
import core.game.node.entity.player.Player
import core.net.packet.IoBuffer
import core.tools.StringUtils
import java.io.PrintWriter
import java.io.StringWriter

/** The social decoders and the dispatch of Decoders530, as the original has them. */
enum class Decoders530(val opcode: Int) {
/* ORIGINAL_DECODERS_SOCIAL */
/* ORIGINAL_DECODER_CHAT_SETTINGS */
/* ORIGINAL_DECODER_CHAT_MESSAGE */
    ;
    abstract fun decode(player: Player, buffer: IoBuffer): Packet

/* ORIGINAL_DECODERS_COMPANION */
}
''')

adapter('kotlinx/coroutines/Coroutines.kt', '''
// Oracle-only stand-in for the kotlinx.coroutines symbols ManagementEvents.kt
// imports. The pinned compiler artifacts do not include kotlinx-coroutines.
// The launched block is kept; the case runs it on the calling thread when it
// pumps the management queue, and the stand-in for withContext stops the
// block (with an Error the original's catch of Exception does not take)
// instead of blocking on an empty queue. Real dispatcher threads and their
// timing relative to the world tick are not part of this corpus.
package kotlinx.coroutines

import kotlin.coroutines.Continuation
import kotlin.coroutines.CoroutineContext
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine

interface CoroutineScope

object GlobalScope : CoroutineScope

object Dispatchers {
    @JvmStatic
    val IO: CoroutineContext = EmptyCoroutineContext
}

fun CoroutineScope.launch(block: suspend CoroutineScope.() -> Unit) {
    val scope = this
    oracle.Support.launch(Runnable {
        var outcome: Result<Unit>? = null
        block.startCoroutine(scope, Continuation(EmptyCoroutineContext) { outcome = it })
        val finished = outcome ?: throw IllegalStateException("oracle coroutine adapter cannot suspend")
        finished.getOrThrow()
    })
}

suspend fun <T> withContext(context: CoroutineContext, block: suspend CoroutineScope.() -> T): T {
    oracle.Support.beforeTake()
    return block(GlobalScope)
}
''')
