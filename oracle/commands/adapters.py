"""Adapters for the commands oracle.

Every class here stands for a system the command, quick chat and abuse report slice does not port. It records the
call through oracle.Support and answers from the case; `/* ORIGINAL_* */` markers are replaced by text taken from the
pinned sources when the oracle is generated. No adapter exposes a public field a Kotlin property read could take in
place of a getter the original calls: the only public field is the one the original declares public
(`PlayerDetails.accountInfo`).
"""

ADAPTERS = {}


def adapter(path, text):
    ADAPTERS[path] = text.lstrip('\n')


adapter('core/ServerConstants.kt', '''
package core

import core.game.world.map.Location
import rs09.game.content.activity.castlewars.CastleWars

/** The constants the selected code reads, set from the case; the destination table is the original's text. */
class ServerConstants {
    companion object {
        @JvmField var PLAYER_COMMANDS = oracle.Support.flag("playerCommands")
        @JvmField var I_AM_A_CHEATER = oracle.Support.flag("cheater")
        @JvmField var HOME_LOCATION: Location? = oracle.Support.home()
        @JvmField var DATA_PATH: String? = oracle.Support.dataPath()
        /* ORIGINAL_TELEPORT_DESTINATIONS */
    }
}
''')

adapter('rs09/game/content/activity/castlewars/CastleWars.kt', '''
package rs09.game.content.activity.castlewars

import core.game.world.map.Location

/** The lobby bank area's random location, drawn once when ServerConstants initialises: a case input. */
object CastleWars {
    class Area {
        val randomLoc: Location
            get() = oracle.Support.castleWars()
    }

    val lobbyBankArea = Area()
}
''')

adapter('core/game/world/map/RegionManager.kt', '''
package core.game.world.map

/** Imported by TeleportCommandSet; not used. */
object RegionManager
''')

adapter('core/ServerStore.kt', '''
package core

import org.json.simple.JSONObject

/** The archive store is not ported here: addToList is a recorded call naming the archive it was asked of. */
class ServerStore {
    companion object {
        @JvmStatic
        fun getArchive(name: String): JSONObject {
            val archive = JSONObject()
            archive["oracle-archive"] = name
            return archive
        }

        fun JSONObject.addToList(key: String, value: Any) {
            oracle.Support.emit("store", this["oracle-archive"], key, value)
        }
    }
}
''')

adapter('core/game/node/Node.java', '''
package core.game.node;

/** A node of the case: its key names it in the events. */
public class Node {
    public String key;
}
''')

adapter('core/game/node/entity/Entity.java', '''
package core.game.node.entity;

import core.game.node.Node;
import core.game.node.entity.impl.Properties;
import core.game.world.map.Location;
import core.game.world.update.UpdateMasks;

/** Attributes, chat, location, properties and masks are other systems: every access is recorded or read from the case. */
public class Entity extends Node {
    private final Properties properties = new Properties(this);
    private final UpdateMasks updateMasks = new UpdateMasks(this);

    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key, T fail) {
        return (T) oracle.Support.getAttribute(this, key, fail);
    }

    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String key) {
        return (T) oracle.Support.getAttribute(this, key, null);
    }

    public void setAttribute(String key, Object value) {
        oracle.Support.emit("setAttribute", key(), key, oracle.Support.value(value));
    }

    public void sendChat(String string) {
        oracle.Support.emit("chat", key(), string);
    }

    public Location getLocation() {
        return oracle.Support.location(this);
    }

    public Properties getProperties() {
        return properties;
    }

    public UpdateMasks getUpdateMasks() {
        return updateMasks;
    }

    String key() {
        return key;
    }
}
''')

adapter('core/game/node/entity/impl/Properties.java', '''
package core.game.node.entity.impl;

import core.game.node.entity.Entity;
import core.game.world.map.Location;

/** Only the teleport location is written by the commands: a recorded call. */
public final class Properties {
    private final Entity owner;

    public Properties(Entity owner) {
        this.owner = owner;
    }

    public Location getTeleportLocation() {
        throw new UnsupportedOperationException("the commands only write the teleport location");
    }

    public void setTeleportLocation(Location location) {
        oracle.Support.emit("teleportLocation", owner.key, oracle.Support.coord(location));
    }
}
''')

adapter('core/game/world/update/UpdateMasks.java', '''
package core.game.world.update;

import core.game.node.entity.Entity;
import core.game.world.update.flag.EntityFlag;
import core.game.world.update.flag.context.ChatMessage;

/** The mask registry is world/update_masks: a registration is recorded with the chat context's fields. */
public final class UpdateMasks {
    private final Entity owner;

    public UpdateMasks(Entity owner) {
        this.owner = owner;
    }

    public boolean register(EntityFlag flag, Object context) {
        ChatMessage chat = (ChatMessage) context;
        oracle.Support.emit("registerChat", owner.key, flag.name(), chat.getPlayer().key, chat.getText(), chat.getEffects(),
            chat.getNumChars(), chat.getChatIcon(), chat.isQuickChat);
        return true;
    }
}
''')

adapter('core/game/world/update/flag/EntityFlag.java', '''
package core.game.world.update.flag;

/** The flag the quick chat pulse registers. */
public enum EntityFlag {
    Chat
}
''')

adapter('core/game/node/entity/player/Player.java', '''
package core.game.node.entity.player;

import content.global.skill.slayer.SlayerManager;
import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.info.Rights;
import core.game.node.entity.player.link.PacketDispatch;
import core.game.node.entity.player.link.SavedData;
import core.game.node.entity.player.link.TeleportManager;
import core.game.node.entity.skill.Skills;
import core.game.system.communication.CommunicationInfo;
import core.game.world.map.zone.ZoneMonitor;

/** A case player. Getters answer from the case; the calls the commands make are recorded. getRights is the original. */
public class Player extends Entity {
    private final PlayerDetails details;
    private final String name;
    private final String username;
    private final PacketDispatch dispatch = new PacketDispatch(this);
    private final ZoneMonitor zones = new ZoneMonitor(this);
    private final TeleportManager teleporter = new TeleportManager(this);
    private final Skills skills = new Skills(this);
    private final CommunicationInfo communication = new CommunicationInfo(this);
    private final Container inventory = new Container(this, "inventory");
    private final Container bank = new Container(this, "bank");
    private final Container equipment = new Container(this, "equipment");
    private final Container bankPrimary = new Container(this, "bankPrimary");
    private final Container bankSecondary = new Container(this, "bankSecondary");

    public Player(String key, String name, String username, PlayerDetails details) {
        this.key = key;
        this.name = name;
        this.username = username;
        this.details = details;
    }

    public PlayerDetails getDetails() {
        return details;
    }

    /* ORIGINAL_PLAYER_GET_RIGHTS */

    public String getName() {
        return name;
    }

    public String getUsername() {
        return username;
    }

    public void clear() {
        oracle.Support.emit("clear", key);
    }

    public void sendMessage(String message) {
        oracle.Support.emit("message", key, message);
    }

    public PacketDispatch getPacketDispatch() {
        return dispatch;
    }

    public ZoneMonitor getZoneMonitor() {
        return zones;
    }

    public TeleportManager getTeleporter() {
        return teleporter;
    }

    public Skills getSkills() {
        return skills;
    }

    public CommunicationInfo getCommunication() {
        return communication;
    }

    public SavedData getSavedData() {
        throw new UnsupportedOperationException("Rule.canRequest is not reached by the commands oracle");
    }

    public Container getInventory() {
        return inventory;
    }

    public Container getBank() {
        return bank;
    }

    public Container getEquipment() {
        return equipment;
    }

    public Container getBankPrimary() {
        return bankPrimary;
    }

    public Container getBankSecondary() {
        return bankSecondary;
    }
}
''')

adapter('core/game/node/entity/player/link/SavedData.java', '''
package core.game.node.entity.player.link;

/** Only for Rule.canRequest to compile; no case reaches it. */
public final class SavedData {
    public GlobalData getGlobalData() {
        throw new UnsupportedOperationException();
    }
}
''')

adapter('core/game/node/entity/player/link/GlobalData.java', '''
package core.game.node.entity.player.link;

/** Only for Rule.canRequest to compile; no case reaches it. */
public final class GlobalData {
    public long getChatPing() {
        throw new UnsupportedOperationException();
    }
}
''')

adapter('core/game/node/entity/player/info/PlayerDetails.java', '''
package core.game.node.entity.player.info;

import core.auth.UserAccountInfo;

/** The account field and getRights as the original has them; the account object is the case's. */
public class PlayerDetails {
    public UserAccountInfo accountInfo;

    public PlayerDetails(UserAccountInfo accountInfo) {
        this.accountInfo = accountInfo;
    }

    /* ORIGINAL_DETAILS_GET_RIGHTS */
}
''')

adapter('core/game/node/entity/player/info/LogType.java', '''
package core.game.node.entity.player.info;

/** The log type the Command branch passes. */
public enum LogType {
    COMMAND
}
''')

adapter('core/game/node/entity/player/info/PlayerMonitor.java', '''
package core.game.node.entity.player.info;

import core.game.node.entity.player.Player;

/** PlayerMonitor is not ported here: a recorded call. */
public final class PlayerMonitor {
    public static void log(Player player, LogType type, String details) {
        oracle.Support.emit("monitor", player.key, type.name(), details);
    }
}
''')

adapter('core/game/node/entity/player/link/PacketDispatch.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;

/** The game-message packet is not ported here: a recorded call. */
public final class PacketDispatch {
    private final Player owner;

    public PacketDispatch(Player owner) {
        this.owner = owner;
    }

    public void sendMessage(String message) {
        oracle.Support.emit("packetMessage", owner.key, message);
    }
}
''')

adapter('core/game/node/entity/player/link/TeleportManager.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.Entity;
import core.game.world.map.Location;

/** world/teleport is the native TeleportManager; here send is a recorded call answered by the case. */
public final class TeleportManager {
    public enum TeleportType {
        NORMAL
    }

    private final Entity owner;

    public TeleportManager(Entity owner) {
        this.owner = owner;
    }

    public boolean send(Location location, TeleportType type) {
        oracle.Support.emit("teleport", owner.key, oracle.Support.coord(location), type.name());
        return oracle.Support.answerBool("teleport", true);
    }
}
''')

adapter('core/game/world/map/zone/ZoneMonitor.java', '''
package core.game.world.map.zone;

import core.game.node.entity.player.Player;

/** The zones' command hooks are content: a recorded call answered by the case. */
public final class ZoneMonitor {
    private final Player owner;

    public ZoneMonitor(Player owner) {
        this.owner = owner;
    }

    public boolean parseCommand(Player player, String name, String[] arguments) {
        oracle.Support.emit("zone", player.key, name, java.util.Arrays.asList(arguments));
        return oracle.Support.answerBool("zone", false);
    }
}
''')

adapter('core/game/node/entity/skill/Skills.java', '''
package core.game.node.entity.skill;

import static java.lang.Math.floor;

import core.game.node.entity.Entity;

/** getSkillByName, SKILL_NAME and getExperienceByLevel are the original's; experience and levels are case answers. */
public final class Skills {
    /* ORIGINAL_SKILL_NAME */

    private final Entity owner;

    public Skills(Entity owner) {
        this.owner = owner;
    }

    public double getExperience(int slot) {
        oracle.Support.emit("experience", owner.key, slot);
        return oracle.Support.answerDouble("experience:" + owner.key + ":" + slot);
    }

    public int getStaticLevel(int slot) {
        oracle.Support.emit("staticLevel", owner.key, slot);
        return oracle.Support.answerInt("staticLevel:" + owner.key + ":" + slot, 1);
    }

    /* ORIGINAL_GET_EXPERIENCE_BY_LEVEL */

    /* ORIGINAL_GET_SKILL_BY_NAME */
}
''')

adapter('content/global/skill/slayer/SlayerManager.java', '''
package content.global.skill.slayer;

import core.game.node.entity.player.Player;

/** Slayer is content: the assignment's amount is a recorded case answer. */
public final class SlayerManager {
    private final Player player;

    private SlayerManager(Player player) {
        this.player = player;
    }

    public static SlayerManager getInstance(Player player) {
        return new SlayerManager(player);
    }

    public int getAmount() {
        oracle.Support.emit("slayerAmount", player.key);
        return oracle.Support.answerInt("slayerAmount:" + player.key, 0);
    }
}
''')

adapter('core/game/system/communication/CommunicationInfo.java', '''
package core.game.system.communication;

import core.game.node.entity.player.Player;

/** world/social owns clans; the player's clan is a case answer (null when it has none). */
public final class CommunicationInfo {
    private final Player owner;

    public CommunicationInfo(Player owner) {
        this.owner = owner;
    }

    public ClanEntry getClan() {
        Object owner = oracle.Support.answer("clan:" + this.owner.key, null);
        return owner == null ? null : new ClanEntry((String) owner);
    }
}
''')

adapter('core/game/system/communication/ClanEntry.java', '''
package core.game.system.communication;

/** A clan, by its owner. */
public final class ClanEntry {
    private final String owner;

    ClanEntry(String owner) {
        this.owner = owner;
    }

    public String getOwner() {
        return owner;
    }
}
''')

adapter('core/game/container/Container.java', '''
package core.game.container;

import core.game.node.entity.player.Player;
import core.game.node.item.Item;

/** Containers are inventory/: every call the commands make is recorded and answered by the case. */
public class Container {
    private final Player owner;
    private final String name;

    public Container(Player owner, String name) {
        this.owner = owner;
        this.name = name;
    }

    String at() {
        return owner.key + ":" + name;
    }

    public boolean isFull() {
        oracle.Support.emit("isFull", owner.key, name);
        return oracle.Support.answerBool("isFull:" + at(), false);
    }

    public boolean add(Item item) {
        oracle.Support.emit("add", owner.key, name, item.getId(), item.getAmount());
        return oracle.Support.answerBool("add", true);
    }

    public boolean remove(Item item) {
        oracle.Support.emit("remove", owner.key, name, item.getId(), item.getAmount());
        return oracle.Support.answerBool("remove", true);
    }

    public Item getItem(Item item) {
        oracle.Support.emit("getItem", owner.key, name, item.getId(), item.getAmount());
        Object amount = oracle.Support.answer("getItem:" + at() + ":" + item.getId(), null);
        return amount == null ? null : new Item(item.getId(), ((Number) amount).intValue());
    }

    public int getAmount(int id) {
        oracle.Support.emit("amount", owner.key, name, id);
        return oracle.Support.answerInt("amount:" + at() + ":" + id, 0);
    }

    public int getAmount(Item item) {
        oracle.Support.emit("amountItem", owner.key, name, item.getId());
        return oracle.Support.answerInt("amountItem:" + at() + ":" + item.getId(), 0);
    }

    public Item[] toArray() {
        oracle.Support.emit("slots", owner.key, name);
        return oracle.Support.slots(this, owner, name);
    }

    public Item get(int slot) {
        oracle.Support.emit("get", owner.key, name, slot);
        Item[] items = oracle.Support.slots(this, owner, name);
        return slot >= 0 && slot < items.length ? items[slot] : null;
    }

    public Item replace(Item item, int slot) {
        oracle.Support.emit("replace", owner.key, name, item.getId(), slot);
        return null;
    }
}
''')

adapter('core/game/node/item/Item.java', '''
package core.game.node.item;

/** An item value; its name and note change are definition lookups recorded and answered by the case. */
public class Item {
    private final int id;
    private int amount;
    private int charge;
    private int slot;
    private String container;
    private String owner;

    public Item(int id) {
        this(id, 1);
    }

    public Item(int id, int amount) {
        this.id = id;
        this.amount = amount;
    }

    public static Item inSlot(int id, int slot, int charge, String owner, String container) {
        Item item = new Item(id, 1);
        item.slot = slot;
        item.charge = charge;
        item.owner = owner;
        item.container = container;
        return item;
    }

    public int getId() {
        return id;
    }

    public int getAmount() {
        return amount;
    }

    public void setAmount(int amount) {
        this.amount = amount;
    }

    public int getSlot() {
        return slot;
    }

    public int getCharge() {
        return charge;
    }

    public void setCharge(int charge) {
        oracle.Support.emit("setCharge", owner, container, slot, charge);
        this.charge = charge;
    }

    public String getName() {
        oracle.Support.emit("itemName", id);
        return (String) oracle.Support.answer("itemName:" + id, "item " + id);
    }

    public int getNoteChange() {
        oracle.Support.emit("noteChange", id);
        return oracle.Support.answerInt("noteChange:" + id, id);
    }
}
''')

adapter('core/cache/def/impl/ItemDefinition.java', '''
package core.cache.def.impl;

import java.util.AbstractMap;
import java.util.Collections;
import java.util.Map;
import java.util.Set;

/** The definition table is definitions/: its size and names are recorded case answers. */
public final class ItemDefinition {
    private final int id;

    private ItemDefinition(int id) {
        this.id = id;
    }

    public static Map<Integer, ItemDefinition> getDefinitions() {
        oracle.Support.emit("itemDefs");
        int size = oracle.Support.answerInt("itemDefs", 0);
        return new AbstractMap<Integer, ItemDefinition>() {
            @Override public Set<Map.Entry<Integer, ItemDefinition>> entrySet() { return Collections.emptySet(); }
            @Override public int size() { return size; }
        };
    }

    public static ItemDefinition forId(int id) {
        oracle.Support.emit("itemDef", id);
        return oracle.Support.answerBool("itemDefExists:" + id, true) ? new ItemDefinition(id) : null;
    }

    public String getName() {
        return (String) oracle.Support.answer("itemDefName:" + id, null);
    }
}
''')

adapter('core/cache/def/impl/NPCDefinition.java', '''
package core.cache.def.impl;

/** Imported by QCRepository; not used. */
public final class NPCDefinition {
}
''')

adapter('core/cache/def/impl/DataMap.java', '''
package core.cache.def.impl;

/** Data maps are cache index 17 (native: startup::loaders::music::DataMap): getString is a recorded case answer. */
public final class DataMap {
    private final int id;

    private DataMap(int id) {
        this.id = id;
    }

    public static DataMap get(int id) {
        return new DataMap(id);
    }

    public String getString(int key) {
        oracle.Support.emit("dataMap", id, key);
        Object value = oracle.Support.answer("dataMap:" + id + ":" + key, null);
        return (String) value;
    }
}
''')

adapter('core/cache/Cache.java', '''
package core.cache;

/** The cache: index 24's files are case answers (hex), recorded when read. */
public final class Cache {
    public static final class Index {
        private final int index;

        Index(int index) {
            this.index = index;
        }

        public byte[] getFileData(int archive, int file) {
            oracle.Support.emit("qcFile", index, archive, file);
            return oracle.Support.bytes("qcFile:" + file);
        }
    }

    public static Index[] getIndexes() {
        Index[] out = new Index[30];
        for (int i = 0; i < out.length; i++) out[i] = new Index(i);
        return out;
    }
}
''')

adapter('core/game/system/SystemManager.java', '''
package core.game.system;

/** The system manager is not ported here: the updater and the flag are recorded calls. */
public final class SystemManager {
    public static final class Updater {
        public void setCountdown(int seconds) {
            oracle.Support.emit("countdown", seconds);
        }

        public void cancel() {
            oracle.Support.emit("cancelUpdate");
        }
    }

    private static final Updater UPDATER = new Updater();

    public static Updater getUpdater() {
        return UPDATER;
    }

    public static void flag(SystemState state) {
        oracle.Support.emit("flag", state.name());
    }
}
''')

adapter('core/game/system/SystemState.java', '''
package core.game.system;

/** The states SystemManager.flag takes. */
public enum SystemState {
    ACTIVE, UPDATING, PRIVATE, TERMINATED
}
''')

adapter('core/game/system/task/Pulse.java', '''
package core.game.system.task;

import core.game.node.Node;

/** A pulse: the world runs it; here the case runs it by its submission index. */
public abstract class Pulse {
    private final int delay;

    public Pulse(int delay) {
        this.delay = delay;
    }

    public Pulse(int delay, Node... checks) {
        this.delay = delay;
    }

    public int getDelay() {
        return delay;
    }

    public abstract boolean pulse();
}
''')

adapter('core/integrations/discord/Discord.java', '''
package core.integrations.discord;

/** The Discord integration: a recorded call. */
public final class Discord {
    public static void postPlayerAlert(String player, String type) {
        oracle.Support.emit("discord", player, type);
    }
}
''')

adapter('content/data/ChargedItem.kt', '''
package content.data

/** ChargedItem is content data: forId, forCharge and getCharge are recorded case answers. */
class ChargedItem(private val id: Int) {
    fun forCharge(charge: Int): Int = oracle.Support.forCharge(id, charge)

    companion object {
        @JvmStatic
        fun forId(id: Int): ChargedItem? = if (oracle.Support.chargedItem(id)) ChargedItem(id) else null

        @JvmStatic
        fun getCharge(id: Int): Int? = oracle.Support.chargedCharge(id)
    }
}
''')

adapter('core/game/world/map/Location.kt', '''
package core.game.world.map

/** A location value with the equality the original's has (x, y and plane). */
class Location(val x: Int, val y: Int, val z: Int) {
    override fun equals(other: Any?): Boolean = other is Location && other.x == x && other.y == y && other.z == z
    override fun hashCode(): Int = (z shl 30) or (x shl 15) or y

    companion object {
        @JvmStatic
        fun create(x: Int, y: Int, z: Int): Location = Location(x, y, z)
    }
}
''')

adapter('core/game/world/GameWorld.kt', '''
package core.game.world

import core.auth.UserAccountInfo
import core.game.node.entity.player.Player
import core.game.system.task.Pulse

/** The world's settings, pulse runner, account storage and authenticator: answered and recorded by the case. */
object GameWorld {
    class Settings(val isDevMode: Boolean, val isBeta: Boolean)

    class Runner {
        fun submit(pulse: Pulse) {
            oracle.Support.submit(pulse)
        }
    }

    class Storage {
        fun checkUsernameTaken(username: String): Boolean {
            oracle.Support.emit("taken", username)
            return oracle.Support.answerBool("taken:" + username, false)
        }

        fun getAccountInfo(username: String): UserAccountInfo {
            oracle.Support.emit("accountInfo", username)
            return oracle.Support.account(username)
        }

        fun getUsernamesWithIP(ip: String): List<String> {
            oracle.Support.emit("ips", ip)
            return oracle.Support.names("ips:" + ip)
        }

        fun update(info: UserAccountInfo) {
            oracle.Support.emit("accountUpdate", oracle.Support.accountKey(info))
        }
    }

    class Authenticator {
        fun checkPassword(player: Player, password: String): Boolean {
            oracle.Support.emit("checkPassword", player.key, password)
            return oracle.Support.answerBool("checkPassword", false)
        }

        fun updatePassword(username: String, newPassword: String) {
            oracle.Support.emit("updatePassword", username, newPassword)
        }
    }

    @JvmStatic
    var settings: Settings? = oracle.Support.settings()

    @JvmStatic
    var Pulser = Runner()

    @JvmStatic
    val accountStorage = Storage()

    @JvmStatic
    val authenticator = Authenticator()
}
''')

adapter('core/game/world/repository/Repository.kt', '''
package core.game.world.repository

import core.game.node.entity.player.Player

/** The repository's name map is world/repository: a recorded call answered by the case. */
object Repository {
    @JvmStatic
    fun getPlayerByName(name: String?): Player? {
        oracle.Support.emit("byName", name)
        return oracle.Support.player(oracle.Support.answer("byName:" + name, null))
    }
}
''')

adapter('core/api/ContentAPI.kt', '''
package core.api

import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.tools.Log

/** The ContentAPI members the selected code calls: recorded calls; handlers and tasks are kept for the case to run. */
fun sendMessage(player: Player, message: String) {
    oracle.Support.emit("apiMessage", player.key, message)
}

fun sendChat(entity: Entity, message: String, delay: Int = -1) {
    if (delay > -1) throw UnsupportedOperationException("no selected caller passes a delay")
    entity.sendChat(message)
}

fun sendDialogue(player: Player, message: String) {
    oracle.Support.emit("dialogue", player.key, message)
}

fun sendInputDialogue(player: Player, type: InputType, prompt: String, handler: (value: Any) -> Unit) {
    oracle.Support.emit("input", player.key, type.name, prompt)
    oracle.Support.inputs.add(handler)
}

fun runTask(entity: Entity, delay: Int = 0, repeatTimes: Int = 1, task: () -> Unit) {
    oracle.Support.emit("runTask", entity.key, delay, repeatTimes)
    oracle.Support.tasks.add(task)
}

fun <T> getAttribute(entity: Entity, attribute: String, default: T): T {
    oracle.Support.emit("apiGetAttribute", entity.key, attribute, oracle.Support.value(default))
    @Suppress("UNCHECKED_CAST")
    return oracle.Support.storedAttribute(entity, attribute, default) as T
}

fun <T> setAttribute(entity: Entity, attribute: String, value: T) {
    oracle.Support.emit("apiSetAttribute", entity.key, attribute, oracle.Support.value(value))
}

fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", origin.name, type.name, message)
}

/** A slayer task: its enum name. */
class OracleTask(val name: String)

fun getSlayerTask(player: Player): OracleTask? {
    oracle.Support.emit("slayerTask", player.key)
    val name = oracle.Support.answer("slayerTask:" + player.key, null) as String?
    return if (name == null) null else OracleTask(name)
}
''')

adapter('core/tools/SystemLogger.kt', '''
package core.tools

/** Imported by the quick chat sources; logging goes through core.api.log. */
object SystemLogger

/* ORIGINAL_LOG_ENUM */
''')

adapter('core/worker/ManagementEvents.kt', '''
package core.worker

import com.google.protobuf.Message
import proto.management.ClanMessage

/** The management queue is world/social's: a publication is recorded with the message's fields. */
object ManagementEvents {
    @JvmStatic
    fun publish(event: Message) {
        val clan = event as ClanMessage
        oracle.Support.emit("publish", clan.sender, clan.clanName, clan.message, clan.rank)
    }
}
''')

adapter('kotlinx/coroutines/Coroutines.kt', '''
// Oracle-only stand-in for the kotlinx.coroutines symbols the command sets import. The launched block is kept and
// the case runs it on the calling thread; withContext runs its block in place. Real dispatcher threads are not part
// of this corpus.
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
    oracle.Support.emit("launch")
    oracle.Support.jobs.add(Runnable {
        var outcome: Result<Unit>? = null
        block.startCoroutine(scope, Continuation(EmptyCoroutineContext) { outcome = it })
        val finished = outcome ?: throw IllegalStateException("oracle coroutine adapter cannot suspend")
        finished.getOrThrow()
    })
}

@OptIn(kotlin.contracts.ExperimentalContracts::class)
suspend fun <T> withContext(context: CoroutineContext, block: suspend CoroutineScope.() -> T): T {
    kotlin.contracts.contract { callsInPlace(block, kotlin.contracts.InvocationKind.EXACTLY_ONCE) }
    return block(GlobalScope)
}
''')

adapter('core/net/packet/in/Decoders530.kt', '''
package core.net.packet.`in`

import core.api.log
import core.game.node.entity.player.Player
import core.net.packet.IoBuffer
import core.tools.Log
import core.tools.StringUtils
import java.io.PrintWriter
import java.io.StringWriter
import java.nio.BufferUnderflowException

/** The three decoders of this slice and the companion's process, as the original has them. */
enum class Decoders530(val opcode: Int) {
    /* ORIGINAL_DECODERS */;

    abstract fun decode(player: Player, buffer: IoBuffer): Packet

/* ORIGINAL_DECODERS_COMPANION */
}
''')

adapter('core/net/packet/PacketProcessor.kt', '''
package core.net.packet

import core.api.sendMessage
import core.game.node.entity.player.info.LogType
import core.game.node.entity.player.info.PlayerMonitor
import core.game.system.command.CommandSystem
import core.game.world.GameWorld
import core.net.packet.`in`.Packet

/** The Command, QuickChat and ReportAbuse branches of process, as the original has them. */
object PacketProcessor {
    @JvmStatic
    fun process(pkt: Packet) {
        when (pkt) {
            /* ORIGINAL_PROCESS_ARMS */
            else -> throw IllegalArgumentException(pkt.toString())
        }
    }
}
''')
