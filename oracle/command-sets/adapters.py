"""Adapters for the command-sets oracle.

Every class here stands for a system the command sets and the system manager do not port. It records the call through
oracle.Support and answers from the case; `/* ORIGINAL_* */` markers are replaced by text taken from the pinned sources
when the oracle is generated. Copied from oracle/commands/adapters.py where the same stand-in serves (accounts,
containers, items, charged items, the coroutine stand-in, the legacy plugins' zone hook), extended where these sets
reach further. No adapter exposes a public field a Kotlin property read could take in place of a getter the original
calls: the only public fields are the ones the originals declare public (`PlayerDetails.accountInfo`, `Pulse.running`).
"""

ADAPTERS = {}


def adapter(path, text):
    ADAPTERS[path] = text.lstrip('\n')


adapter('core/ServerConstants.kt', '''
package core

import core.game.world.map.Location

/** The constants the selected code reads, set from the case. */
class ServerConstants {
    companion object {
        @JvmField var PLAYER_COMMANDS = oracle.Support.flag("playerCommands")
        @JvmField var I_AM_A_CHEATER = oracle.Support.flag("cheater")
        @JvmField var HOME_LOCATION: Location? = oracle.Support.home()
        @JvmField var DATA_PATH: String? = oracle.Support.dataPath()
        @JvmField var BANK_SIZE: Int = oracle.Support.number("bankSize", 496)
        @JvmField var NEW_PLAYER_ANNOUNCEMENT = oracle.Support.flag("newPlayerAnnouncement")
        @JvmField var PLAYER_ATTRIBUTE_PATH: String? = null
    }
}
''')

adapter('core/Server.kt', '''
package core

/** The networking side Server.main sets up: every call the termination makes is recorded. */
object Server {
    class Reactor {
        fun terminate() {
            oracle.Support.emit("reactorTerminate")
        }
    }

    class WebSocket {
        fun stop(timeout: Int, reason: String) {
            oracle.Support.emit("websocketStop", timeout, reason)
        }
    }

    @JvmStatic
    fun setRunning(running: Boolean) {
        oracle.Support.emit("serverRunning", running)
    }

    @JvmStatic
    fun getReactor(): Reactor = Reactor()

    private var socket: WebSocket? = if (oracle.Support.flag("websocket")) WebSocket() else null

    @JvmStatic
    fun getWebSocketServer(): WebSocket? = socket

    @JvmStatic
    fun setWebSocketServer(server: WebSocket?) {
        oracle.Support.emit("websocketSet", server != null)
        socket = server
    }
}
''')

adapter('core/api/PersistWorld.kt', '''
package core.api

/** A world store saved at termination. */
interface PersistWorld {
    fun parse()
    fun save()
}
''')

adapter('core/api/ShutdownListener.kt', '''
package core.api

/** A shutdown listener. */
interface ShutdownListener {
    fun shutdown()
}
''')

adapter('core/ServerStore.kt', '''
package core

import core.api.PersistWorld

/** The archive store is not ported here: its save is a recorded call. */
class ServerStore : PersistWorld {
    override fun parse() {
        oracle.Support.emit("persistParse", "ServerStore")
    }

    override fun save() {
        oracle.Support.emit("persistSave", "ServerStore")
    }
}
''')

adapter('core/game/bots/AIRepository.kt', '''
package core.game.bots

/** The bot repository: clearAllBots is recorded; PulseRepository answers lookups from the case. */
object AIRepository {
    class BotPulse(private val label: String) {
        val botScript: Any = object {
            override fun toString(): String = label
        }
    }

    class Lookup : java.util.HashMap<String, BotPulse>() {
        override fun get(key: String): BotPulse? {
            val text = oracle.Support.ask("botLookup", null, key) as String?
            return if (text == null) null else BotPulse(text)
        }
    }

    @JvmStatic
    val PulseRepository = Lookup()

    @JvmStatic
    fun clearAllBots() {
        oracle.Support.emit("clearBots")
    }
}
''')

adapter('core/game/node/Node.java', '''
package core.game.node;

/** A node of the case: its key names it in the events. */
public class Node {
    public String key;

    public boolean isActive() {
        return oracle.Support.askBool("nodeActive", true, key);
    }
}
''')

adapter('core/game/node/entity/Entity.java', '''
package core.game.node.entity;

import core.game.node.Node;
import core.game.node.entity.impl.GameAttributes;
import core.game.node.entity.impl.Properties;
import core.game.node.entity.lock.ActionLocks;
import core.game.node.entity.skill.Skills;
import core.game.world.map.Direction;
import core.game.world.map.Location;
import java.util.AbstractMap;
import java.util.Map;
import java.util.Set;

/**
 * Attributes live in the original GameAttributes; each Entity-level call (which in the original also dispatches the
 * attribute event) is one recorded call. Location and direction are the case's.
 */
public class Entity extends Node {
    private final GameAttributes attributes = new GameAttributes();
    private final Properties properties = new Properties(this);
    private final Skills skills = new Skills(this);
    private final ActionLocks locks = new ActionLocks(this);
    protected Location location;
    protected Direction direction = Direction.SOUTH;

    public GameAttributes oracleAttributes() {
        return attributes;
    }

    public void setAttribute(String key, Object value) {
        oracle.Support.emit("setAttribute", this.key, key, oracle.Support.value(value));
        attributes.setAttribute(key, value);
    }

    public <T> T getAttribute(String key) {
        oracle.Support.emit("getAttribute", this.key, key, null);
        return attributes.getAttribute(key);
    }

    public <T> T getAttribute(String string, T fail) {
        oracle.Support.emit("getAttribute", key, string, oracle.Support.value(fail));
        return attributes.getAttribute(string, fail);
    }

    public void removeAttribute(String string) {
        oracle.Support.emit("removeAttribute", key, string);
        attributes.removeAttribute(string);
    }

    /** The raw map: containsKey is the one use, recorded. */
    public Map<String, Object> getAttributes() {
        Map<String, Object> raw = attributes.getAttributes();
        String owner = key;
        return new AbstractMap<String, Object>() {
            @Override public Set<Map.Entry<String, Object>> entrySet() { return raw.entrySet(); }
            @Override public boolean containsKey(Object k) {
                oracle.Support.emit("hasAttribute", owner, k);
                return raw.containsKey(k);
            }
        };
    }

    public void sendChat(String string) {
        oracle.Support.emit("chat", key, string);
    }

    public Location getLocation() {
        return location;
    }

    public Direction getDirection() {
        return direction;
    }

    public void setDirection(Direction direction) {
        oracle.Support.emit("setDirection", key, direction.name());
        this.direction = direction;
    }

    public Properties getProperties() {
        return properties;
    }

    public Skills getSkills() {
        return skills;
    }

    public ActionLocks getLocks() {
        return locks;
    }

    public void init() {
        oracle.Support.emit("init", key);
    }
}
''')

adapter('core/game/node/entity/lock/ActionLocks.java', '''
package core.game.node.entity.lock;

import core.game.node.entity.Entity;

/** The entity's locks (world/action_locks): each query is a recorded case answer. */
public final class ActionLocks {
    private final Entity owner;

    public ActionLocks(Entity owner) {
        this.owner = owner;
    }

    public boolean isMovementLocked() {
        return oracle.Support.askBool("movementLocked", false, owner.key);
    }

    public boolean isTeleportLocked() {
        return oracle.Support.askBool("teleportLocked", false, owner.key);
    }

    public boolean isInteractionLocked() {
        return oracle.Support.askBool("interactionLocked", false, owner.key);
    }
}
''')

adapter('core/game/node/entity/impl/Properties.java', '''
package core.game.node.entity.impl;

import core.game.node.entity.Entity;
import core.game.node.entity.combat.equipment.WeaponInterface;

/** The combat properties the commands read and write: recorded calls. */
public final class Properties {
    private final Entity owner;

    public Properties(Entity owner) {
        this.owner = owner;
    }

    public int[] getBonuses() {
        oracle.Support.emit("bonuses", owner.key);
        java.util.List<Object> list = oracle.Support.list("bonuses:" + owner.key);
        int[] out = new int[list.size()];
        for (int i = 0; i < out.length; i++) out[i] = ((Number) list.get(i)).intValue();
        return out;
    }

    public WeaponInterface.AttackStyle getAttackStyle() {
        throw new UnsupportedOperationException("the commands only write the attack style");
    }

    public void setAttackStyle(WeaponInterface.AttackStyle style) {
        oracle.Support.emit("attackStyle", owner.key, style.getStyle(), style.getBonusType());
    }
}
''')

adapter('core/game/node/entity/combat/equipment/WeaponInterface.java', '''
package core.game.node.entity.combat.equipment;

/** The attack style value class and the constant the commands use. */
public final class WeaponInterface {
    /* ORIGINAL_STYLE_CONTROLLED */

    public static class AttackStyle {
        private final int style;
        private final int bonusType;

        public AttackStyle(int style, int bonusType) {
            this.style = style;
            this.bonusType = bonusType;
        }

        public int getStyle() {
            return style;
        }

        public int getBonusType() {
            return bonusType;
        }
    }
}
''')

adapter('core/game/node/entity/combat/CombatStyle.java', '''
package core.game.node.entity.combat;

/** The combat styles a swing handler reports. */
public enum CombatStyle {
    MELEE, RANGE, MAGIC
}
''')

adapter('core/game/node/entity/combat/CombatSwingHandler.java', '''
package core.game.node.entity.combat;

import core.game.node.entity.Entity;

/** combat_swing is native but not bound to a session here: every calculation is a recorded case answer. */
public abstract class CombatSwingHandler {
    private final CombatStyle type;

    protected CombatSwingHandler(CombatStyle type) {
        this.type = type;
    }

    public CombatStyle getType() {
        return type;
    }

    String label() {
        return getClass().getSimpleName();
    }

    public int calculateAccuracy(Entity entity) {
        return oracle.Support.askInt("accuracy", 0, label(), entity.key);
    }

    public int calculateDefence(Entity victim, Entity attacker) {
        return oracle.Support.askInt("defence", 0, label(), victim.key, attacker.key);
    }

    public int calculateHit(Entity entity, Entity victim, double modifier) {
        return oracle.Support.askInt("hit", 0, label(), entity.key, victim.key, String.valueOf(modifier));
    }
}
''')

for name, style in [('MeleeSwingHandler', 'MELEE'), ('RangeSwingHandler', 'RANGE'), ('MagicSwingHandler', 'MAGIC')]:
    adapter(f'core/game/node/entity/combat/{name}.java', f'''
package core.game.node.entity.combat;

/** A swing handler of the style the original's class has. */
public class {name} extends CombatSwingHandler {{
    public {name}() {{
        super(CombatStyle.{style});
    }}
}}
''')

for name, style in [('MagicalMeleeSwingHandler', 'MAGIC'), ('MagicalRangedSwingHandler', 'MAGIC'), ('RangedMeleeSwingHandler', 'RANGE')]:
    adapter(f'core/game/node/entity/combat/{name}.kt', f'''
package core.game.node.entity.combat

/** The original's singleton handler. */
object {name} : CombatSwingHandler(CombatStyle.{style})
''')

adapter('core/game/node/entity/skill/Skills.java', '''
package core.game.node.entity.skill;

import core.game.node.entity.Entity;

/** The skill constants are the original's; levels are case answers. */
public final class Skills {
    /* ORIGINAL_SKILL_CONSTANTS */

    private final Entity owner;

    public Skills(Entity owner) {
        this.owner = owner;
    }

    public int getLevel(int slot) {
        return oracle.Support.askInt("level", 1, owner.key, slot);
    }
}
''')

adapter('core/game/node/entity/npc/NPC.java', '''
package core.game.node.entity.npc;

import core.game.node.entity.Entity;
import core.game.world.map.Location;

/** An NPC: the factory, configuration, spawn and the fields the commands set are recorded calls. */
public class NPC extends Entity {
    private final int id;
    private boolean walks;

    public NPC(int id) {
        this(id, null);
    }

    NPC(int id, Location location) {
        this.id = id;
        this.key = oracle.Support.npcKey();
        this.location = location;
        oracle.Support.emit("newNpc", key, id, oracle.Support.coord(location));
    }

    public static NPC create(int id, Location location, Object... objects) {
        return new NPC(id, location);
    }

    public void initConfig() {
        oracle.Support.emit("initConfig", key);
    }

    public String getName() {
        return oracle.Support.askStr("npcName", "npc " + id, key, id);
    }

    public boolean isRespawn() {
        return true;
    }

    public void setRespawn(boolean respawn) {
        oracle.Support.emit("setRespawn", key, respawn);
    }

    public boolean isWalks() {
        return walks;
    }

    public void setWalks(boolean walks) {
        oracle.Support.emit("setWalks", key, walks);
        this.walks = walks;
    }

    @Override
    public Location getLocation() {
        Object moved = oracle.Support.answer("npcLocation", null, key);
        return moved == null ? location : oracle.Support.location(moved);
    }
}
''')

adapter('core/game/node/entity/player/Player.java', '''
package core.game.node.entity.player;

import content.global.skill.slayer.SlayerManager;
import core.game.container.Container;
import core.game.node.entity.Entity;
import core.game.node.entity.combat.*;
import core.game.node.entity.player.info.PlayerDetails;
import core.game.node.entity.player.info.Rights;
import core.game.node.entity.player.link.*;
import core.game.node.entity.player.link.diary.AchievementDiaryManager;
import core.game.node.entity.player.link.quest.QuestRepository;
import core.game.world.map.Direction;
import core.game.world.map.zone.ZoneMonitor;
import core.game.world.update.flag.PlayerFlags;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

/** A case player. Getters answer from the case; the calls the commands make are recorded. getRights and toggleDebug are the original's. */
public class Player extends Entity {
    private final PlayerDetails details;
    private final String name;
    private final String username;
    private final PacketDispatch dispatch = new PacketDispatch(this);
    private final InterfaceManager interfaces;
    private final ZoneMonitor zones = new ZoneMonitor(this);
    private final DialogueInterpreter dialogues = new DialogueInterpreter(this);
    private final SavedData savedData;
    private final PlayerFlags flags;
    private final AchievementDiaryManager diaries;
    private final QuestRepository quests = new QuestRepository(this);
    private final IronmanManager ironman = new IronmanManager(this);
    private final Container inventory = new Container(this, "inventory");
    private final Container bank = new Container(this, "bank");
    private final Container equipment = new Container(this, "equipment");
    private final Container bankPrimary = new Container(this, "bankPrimary");
    private final Container bankSecondary = new Container(this, "bankSecondary");
    private int version;

    public Player(String key, String name, String username, PlayerDetails details, Map spec) {
        this.key = key;
        this.name = name;
        this.username = username;
        this.details = details;
        this.details.owner = key;
        this.interfaces = new InterfaceManager(this, spec.containsKey("packetCount") ? ((Number) spec.get("packetCount")).intValue() : 0);
        this.location = oracle.Support.location(spec.get("location"));
        if (spec.containsKey("direction")) this.direction = Direction.valueOf((String) spec.get("direction"));
        this.version = spec.containsKey("version") ? ((Number) spec.get("version")).intValue() : 0;
        this.savedData = new SavedData(spec);
        this.flags = new PlayerFlags(oracle.Support.location(spec.get("lastSceneGraph")));
        this.diaries = new AchievementDiaryManager(this, (List) spec.get("diaries"));
    }

    public List<Object> oracleState() {
        return Arrays.asList(version, savedData.getGlobalData().getTestStage(), savedData.getGlobalData().oraclePlaques());
    }

    public PlayerDetails getDetails() {
        return details;
    }

    /* ORIGINAL_PLAYER_GET_RIGHTS */

    /* ORIGINAL_TOGGLE_DEBUG */

    public String getName() {
        return name;
    }

    public String getUsername() {
        return username;
    }

    public int getVersion() {
        return version;
    }

    public void setVersion(int version) {
        oracle.Support.emit("setVersion", key, version);
        this.version = version;
    }

    public boolean isArtificial() {
        return oracle.Support.askBool("artificial", false, key);
    }

    public CombatSwingHandler getSwingHandler(boolean swing) {
        String label = oracle.Support.askStr("swingHandler", "melee", key, swing);
        switch (label) {
            case "range": return new RangeSwingHandler();
            case "magic": return new MagicSwingHandler();
            case "ma_melee": return MagicalMeleeSwingHandler.INSTANCE;
            case "ma_ranged": return MagicalRangedSwingHandler.INSTANCE;
            case "ra_melee": return RangedMeleeSwingHandler.INSTANCE;
            default: return new MeleeSwingHandler();
        }
    }

    public void unlock() {
        oracle.Support.emit("unlock", key);
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

    public core.net.IoSession getSession() {
        return details.getSession();
    }

    public InterfaceManager getInterfaceManager() {
        return interfaces;
    }

    public ZoneMonitor getZoneMonitor() {
        return zones;
    }

    public DialogueInterpreter getDialogueInterpreter() {
        return dialogues;
    }

    public SavedData getSavedData() {
        return savedData;
    }

    public GlobalData getGlobalData() {
        return savedData.getGlobalData();
    }

    public PlayerFlags getPlayerFlags() {
        return flags;
    }

    public AchievementDiaryManager getAchievementDiaryManager() {
        return diaries;
    }

    public QuestRepository getQuestRepository() {
        return quests;
    }

    public IronmanManager getIronmanManager() {
        return ironman;
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

adapter('core/game/node/entity/player/info/PlayerDetails.java', '''
package core.game.node.entity.player.info;

import core.auth.UserAccountInfo;

/** The account field, getRights and the credit accessors as the original has them; the rest are case answers. */
public class PlayerDetails {
    public UserAccountInfo accountInfo;
    public String owner;

    public PlayerDetails(UserAccountInfo accountInfo) {
        this.accountInfo = accountInfo;
    }

    /* ORIGINAL_DETAILS_GET_RIGHTS */

    /* ORIGINAL_DETAILS_CREDITS */

    public String getIpAddress() {
        return oracle.Support.askStr("ipAddress", "127.0.0.1", owner);
    }

    public String getCompName() {
        return oracle.Support.askStr("compName", "pc", owner);
    }

    public void save() {
        oracle.Support.emit("saveDetails", owner);
    }

    public core.net.IoSession getSession() {
        return new core.net.IoSession(owner);
    }
}
''')

adapter('core/game/node/entity/player/info/PlayerMonitor.java', '''
package core.game.node.entity.player.info;

/** PlayerMonitor is not ported here: a recorded call. */
public final class PlayerMonitor {
    public static void flushRemainingEventsImmediately() {
        oracle.Support.emit("monitorFlush");
    }
}
''')

adapter('core/game/node/entity/player/link/PacketDispatch.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;

/** The packet writers the selected code reaches: recorded calls. */
public final class PacketDispatch {
    private final Player owner;

    public PacketDispatch(Player owner) {
        this.owner = owner;
    }

    public void sendMessage(String message) {
        oracle.Support.emit("packetMessage", owner.key, message);
    }

    public void sendString(String string, int interfaceId, int lineId) {
        oracle.Support.emit("sendString", owner.key, string, interfaceId, lineId);
    }

    public void sendVarcUpdate(short index, int value) {
        oracle.Support.emit("varc", owner.key, (int) index, value);
    }

    public void sendInterfaceConfig(int interfaceId, int childId, boolean hide) {
        oracle.Support.emit("interfaceConfig", owner.key, interfaceId, childId, hide);
    }

    public void sendSystemUpdate(int time) {
        oracle.Support.emit("systemUpdate", owner.key, time);
    }
}
''')

adapter('core/game/node/entity/player/link/InterfaceManager.java', '''
package core.game.node.entity.player.link;

import core.game.component.Component;
import core.game.node.entity.player.Player;

/** InterfaceManager is world/interfaces: the queries and calls the commands make are recorded case answers. */
public final class InterfaceManager {
    private final Player owner;
    private int packetCount;

    public InterfaceManager(Player owner, int packetCount) {
        this.owner = owner;
        this.packetCount = packetCount;
    }

    /* ORIGINAL_GET_PACKET_COUNT */

    public boolean isOpened() {
        return oracle.Support.askBool("isOpened", false, owner.key);
    }

    public Component getOpened() {
        Object id = oracle.Support.ask("opened", null, owner.key);
        return id == null ? null : new Component(((Number) id).intValue());
    }

    public Component open(Component component) {
        oracle.Support.emit("open", owner.key, component.getId());
        return component;
    }

    public boolean close() {
        return oracle.Support.askBool("close", true, owner.key);
    }
}
''')

adapter('core/game/node/entity/player/link/DialogueInterpreter.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;

/** world/dialogue: close is a recorded call. */
public final class DialogueInterpreter {
    private final Player owner;

    public DialogueInterpreter(Player owner) {
        this.owner = owner;
    }

    public boolean close() {
        return oracle.Support.askBool("closeDialogue", true, owner.key);
    }
}
''')

adapter('core/game/node/entity/player/link/SavedData.java', '''
package core.game.node.entity.player.link;

import java.util.Map;

/** The saved data the commands reach: the global data. */
public final class SavedData {
    private final GlobalData global;

    public SavedData(Map spec) {
        this.global = new GlobalData(spec);
    }

    public GlobalData getGlobalData() {
        return global;
    }
}
''')

adapter('core/game/node/entity/player/link/GlobalData.java', '''
package core.game.node.entity.player.link;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/** The fields from the case; the accessors are the original's. */
public final class GlobalData {
    private int playerTestStage;
    private boolean[] readPlaques;
    private int[] bossCounters;
    private int barrowsLoots;

    GlobalData(Map spec) {
        playerTestStage = spec.containsKey("testStage") ? ((Number) spec.get("testStage")).intValue() : 0;
        List plaques = (List) spec.getOrDefault("readPlaques", new ArrayList<>());
        readPlaques = new boolean[plaques.size()];
        for (int i = 0; i < readPlaques.length; i++) readPlaques[i] = Boolean.TRUE.equals(plaques.get(i));
        List counters = (List) spec.getOrDefault("bossCounters", new ArrayList<>());
        bossCounters = new int[counters.size()];
        for (int i = 0; i < bossCounters.length; i++) bossCounters[i] = ((Number) counters.get(i)).intValue();
        barrowsLoots = spec.containsKey("barrowsLoots") ? ((Number) spec.get("barrowsLoots")).intValue() : 0;
    }

    public List<Object> oraclePlaques() {
        List<Object> out = new ArrayList<>();
        for (boolean b : readPlaques) out.add(b);
        return out;
    }

    /* ORIGINAL_GLOBAL_DATA */
}
''')

adapter('core/game/node/entity/player/link/IronmanMode.java', '''
package core.game.node.entity.player.link;

/** The original's constants, in order. */
public enum IronmanMode {
    /* ORIGINAL_IRONMAN_NAMES */
}
''')

adapter('core/game/node/entity/player/link/IronmanManager.java', '''
package core.game.node.entity.player.link;

import core.game.node.entity.player.Player;

/** world/ironman: the mode is a recorded case answer. */
public final class IronmanManager {
    private final Player owner;

    public IronmanManager(Player owner) {
        this.owner = owner;
    }

    public IronmanMode getMode() {
        return IronmanMode.valueOf(oracle.Support.askStr("ironmanMode", "NONE", owner.key));
    }
}
''')

adapter('core/game/node/entity/player/link/quest/QuestRepository.java', '''
package core.game.node.entity.player.link.quest;

import core.game.node.entity.player.Player;

/** Quest points are content: a recorded case answer. */
public final class QuestRepository {
    private final Player owner;

    public QuestRepository(Player owner) {
        this.owner = owner;
    }

    public int getPoints() {
        return oracle.Support.askInt("questPoints", 0, owner.key);
    }
}
''')

adapter('core/game/node/entity/player/link/diary/AchievementDiaryManager.java', '''
package core.game.node.entity.player.link.diary;

import core.game.node.entity.player.Player;
import java.util.List;

/** The diaries are content: each diary's task table comes from the case and finishTask is recorded. */
public final class AchievementDiaryManager {
    private final AchievementDiary[] diarys;

    public AchievementDiaryManager(Player owner, List spec) {
        int count = spec == null ? 0 : spec.size();
        diarys = new AchievementDiary[count];
        for (int i = 0; i < count; i++) diarys[i] = new AchievementDiary(owner, i, (List) spec.get(i));
    }

    public AchievementDiary[] getDiarys() {
        return diarys;
    }
}
''')

adapter('core/game/node/entity/player/link/diary/AchievementDiary.java', '''
package core.game.node.entity.player.link.diary;

import core.game.node.entity.player.Player;
import java.util.List;

/** A diary: its levels' task counts from the case. */
public final class AchievementDiary {
    private final Player owner;
    private final int index;
    private final boolean[][] taskCompleted;

    AchievementDiary(Player owner, int index, List levels) {
        this.owner = owner;
        this.index = index;
        taskCompleted = new boolean[levels.size()][];
        for (int i = 0; i < taskCompleted.length; i++) taskCompleted[i] = new boolean[((Number) levels.get(i)).intValue()];
    }

    public boolean[][] getTaskCompleted() {
        return taskCompleted;
    }

    public void finishTask(Player player, int level, int task) {
        oracle.Support.emit("finishTask", player.key, index, level, task);
    }
}
''')

adapter('core/game/world/update/flag/PlayerFlags.java', '''
package core.game.world.update.flag;

import core.game.world.map.Location;

/** The last scene graph base, from the case. */
public final class PlayerFlags {
    private final Location lastSceneGraph;

    public PlayerFlags(Location lastSceneGraph) {
        this.lastSceneGraph = lastSceneGraph;
    }

    public Location getLastSceneGraph() {
        return lastSceneGraph;
    }
}
''')

adapter('core/game/node/entity/player/link/TeleportManager.java', '''
package core.game.node.entity.player.link;

/** Imported by nothing the case drives. */
public final class TeleportManager {
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
        return oracle.Support.askBool("zone", false, player.key, name, java.util.Arrays.asList(arguments));
    }
}
''')

adapter('content/global/skill/slayer/SlayerManager.kt', '''
package content.global.skill.slayer

import core.game.node.entity.player.Player

/** Slayer is content: the completed task count is a recorded case answer. */
class SlayerManager private constructor(private val player: Player) {
    class Flags(private val player: Player) {
        val completedTasks: Int
            get() = oracle.Support.askInt("slayerCompleted", 0, player.key)
    }

    val flags = Flags(player)

    companion object {
        @JvmStatic
        fun getInstance(player: Player): SlayerManager = SlayerManager(player)
    }
}
''')

adapter('content/global/activity/ttrail/TreasureTrailManager.kt', '''
package content.global.activity.ttrail

import core.game.node.entity.player.Player

/** Clue counts are content: a recorded case answer. */
class TreasureTrailManager private constructor(private val player: Player) {
    val completedClues: IntArray
        get() {
            oracle.Support.emit("clues", player.key)
            return oracle.Support.list("clues:" + player.key).map { (it as Number).toInt() }.toIntArray()
        }

    companion object {
        @JvmStatic
        fun getInstance(player: Player): TreasureTrailManager = TreasureTrailManager(player)
    }
}
''')

adapter('content/data/BossKillCounter.kt', '''
package content.data

/** The original's constants, in order (their ordinals index the counters). */
enum class BossKillCounter {
    /* ORIGINAL_BOSS_NAMES */
}
''')

adapter('core/api/utils/PlayerStatsCounter.kt', '''
package core.api.utils

import core.game.node.entity.player.Player

/** The kill and drop counters are a database (content): recorded case answers. */
class PlayerStatsCounter {
    companion object {
        @JvmStatic
        fun getKills(player: Player, npcIds: IntArray): Long =
            oracle.Support.askLong("kills", 0L, player.key, npcIds.toList())

        @JvmStatic
        fun getRareDrops(player: Player, itemId: Int): Long =
            oracle.Support.askLong("rareDrops", 0L, player.key, itemId)
    }
}
''')

adapter('core/api/utils/Permadeath.kt', '''
package core.api.utils

import core.game.node.entity.player.Player

/** Permadeath is content: a recorded call. */
fun permadeath(target: Player) {
    oracle.Support.emit("permadeath", target.key)
}
''')

adapter('content/global/handlers/iface/BookInterface.kt', '''
package content.global.handlers.iface

import core.game.node.entity.player.Player

/** The book interface is content: openBook keeps the display callback for the case to run, pageSetup is recorded. */
class BookInterface {
    companion object {
        /* ORIGINAL_BOOK_CONSTANTS */

        fun openBook(player: Player, bookComponent: Int, displayCallback: (player: Player, pageNum: Int, buttonId: Int) -> Boolean) {
            oracle.Support.emit("openBook", player.key, bookComponent)
            oracle.Support.books.add(arrayOf(player.key, displayCallback))
        }

        fun pageSetup(player: Player, bookComponent: Int, title: String, contents: Array<PageSet>, hasPagination: Boolean = true) {
            val pages = contents.map { set -> set.pages.map { page -> page.lines.map { listOf(it.message, it.child) } } }
            oracle.Support.emit("pageSetup", player.key, bookComponent, title, pages, hasPagination)
        }
    }
}

/* ORIGINAL_BOOK_CLASSES */
''')

adapter('content/global/handlers/iface/RulesAndInfo.kt', '''
package content.global.handlers.iface

import core.game.node.entity.player.Player

/** The rules interface's text is content: a recorded call. */
object RulesAndInfo {
    fun setBaseRulesAndInfo(player: Player): Int = oracle.Support.askInt("rulesText", 0, player.key)
}
''')

adapter('content/global/skill/summoning/familiar/Familiar.java', '''
package content.global.skill.summoning.familiar;

/** The attribute name the command toggles. */
public final class Familiar {
    /* ORIGINAL_INFINITE_SPECIAL_MOVE */
}
''')

adapter('content/global/skill/cooking/fermenting/BrewGrowth.kt', '''
package content.global.skill.cooking.fermenting

/** The brewing timer is content: its vats come from the case and brew is recorded. */
class BrewGrowth(private val owner: String) {
    class Vat(private val owner: String, private val index: Int) {
        fun brew(forceStep: Boolean, forceGood: Boolean, forceMature: Boolean, forceBad: Boolean) {
            oracle.Support.emit("brew", owner, index, forceStep, forceGood, forceMature, forceBad)
        }
    }

    fun getVats(): List<Vat> {
        oracle.Support.emit("vats", owner)
        return (0 until oracle.Support.number("vats", 0)).map { Vat(owner, it) }
    }
}
''')

adapter('content/global/skill/farming/timers/CropGrowth.kt', '''
package content.global.skill.farming.timers

import core.game.node.entity.player.Player

/** The farming timers are content: patches and bins come from the case and every write is recorded. */
class CropGrowth(private val owner: String) {
    class Patch(private val owner: String, private val index: Int) {
        var nextGrowth: Long = 0
            set(value) {
                oracle.Support.emit("nextGrowth", owner, index, value)
                field = value
            }
        var diseaseMod: Int = 0
            set(value) {
                oracle.Support.emit("diseaseMod", owner, index, value)
                field = value
            }
    }

    fun getPatches(): List<Patch> {
        oracle.Support.emit("patches", owner)
        return (0 until oracle.Support.number("patches", 0)).map { Patch(owner, it) }
    }

    fun run(player: Player): Boolean {
        return oracle.Support.askBool("cropRun", true, player.key)
    }
}

class Compost(private val owner: String) {
    class Bin(private val owner: String, private val index: Int) {
        val isFinished: Boolean
            get() = oracle.Support.askBool("binFinished", false, owner, index)
        val isClosed: Boolean
            get() = oracle.Support.askBool("binClosed", false, owner, index)

        fun finish() {
            oracle.Support.emit("binFinish", owner, index)
        }

        fun reset() {
            oracle.Support.emit("binReset", owner, index)
        }
    }

    fun getBins(): List<Bin> {
        oracle.Support.emit("bins", owner)
        return (0 until oracle.Support.number("bins", 0)).map { Bin(owner, it) }
    }
}
''')

adapter('content/minigame/fishingtrawler/TrawlerLoot.kt', '''
package content.minigame.fishingtrawler

import core.game.node.entity.player.Player

/** Trawler loot is content: a recorded call. */
object TrawlerLoot {
    fun addLootAndMessage(player: Player, fishLevel: Int, rolls: Int, skipJunk: Boolean) {
        oracle.Support.emit("trawlerLoot", player.key, fishLevel, rolls, skipJunk)
    }
}
''')

adapter('content/region/misthalin/draynor/quest/anma/AnmaCutscene.kt', '''
package content.region.misthalin.draynor.quest.anma

import core.game.node.entity.player.Player

/** The cutscene is content: its start is a recorded call. */
class AnmaCutscene(private val player: Player) {
    fun start() {
        oracle.Support.emit("cutscene", "AnmaCutscene", player.key)
    }
}
''')

adapter('core/game/component/Component.java', '''
package core.game.component;

/** A component value by id. */
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

adapter('core/game/ge/GrandExchangeOffer.kt', '''
package core.game.ge

/** An offer row from the case: [itemID, amount, offeredValue, sell, amountLeft]. */
class GrandExchangeOffer(row: List<Any?>) {
    var itemID = (row[0] as Number).toInt()
    var amount = (row[1] as Number).toInt()
    var offeredValue = (row[2] as Number).toInt()
    var sell = row[3] as Boolean
    val amountLeft: Int = (row[4] as Number).toInt()
}
''')

adapter('core/game/ge/GrandExchange.kt', '''
package core.game.ge

/** world/grand_exchange is native but not bound here: the offer queries are recorded case answers. */
class GrandExchange {
    companion object {
        @JvmStatic
        fun getValidOffers(): List<GrandExchangeOffer> {
            oracle.Support.emit("validOffers")
            return oracle.Support.list("validOffers").map { GrandExchangeOffer(it as List<Any?>) }
        }

        @JvmStatic
        fun getBotOffers(): List<GrandExchangeOffer> {
            oracle.Support.emit("botOffers")
            return oracle.Support.list("botOffers").map { GrandExchangeOffer(it as List<Any?>) }
        }
    }
}
''')

adapter('core/game/system/communication/CommunicationInfo.java', '''
package core.game.system.communication;

import core.game.node.entity.player.Player;

/** world/social owns private messages: sendMessage is a recorded call. */
public final class CommunicationInfo {
    public static void sendMessage(Player player, String target, String message) {
        oracle.Support.emit("privateMessage", player.key, target, message);
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
        return oracle.Support.askStr("itemName", "item " + id, id);
    }

    public int getNoteChange() {
        return oracle.Support.askInt("noteChange", id, id);
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

    public boolean isFull() {
        return oracle.Support.askBool("isFull", false, owner.key, name);
    }

    public boolean add(Item item) {
        return oracle.Support.askBool("add", true, owner.key, name, item.getId(), item.getAmount());
    }

    public boolean remove(Item item) {
        return oracle.Support.askBool("remove", true, owner.key, name, item.getId(), item.getAmount());
    }

    public Item getItem(Item item) {
        Object amount = oracle.Support.ask("getItem", null, owner.key, name, item.getId(), item.getAmount());
        return amount == null ? null : new Item(item.getId(), ((Number) amount).intValue());
    }

    public int getAmount(int id) {
        return oracle.Support.askInt("amount", 0, owner.key, name, id);
    }

    public int getAmount(Item item) {
        return oracle.Support.askInt("amountItem", 0, owner.key, name, item.getId());
    }

    public int getMaximumAdd(Item item) {
        return oracle.Support.askInt("maximumAdd", Integer.MAX_VALUE, owner.key, name, item.getId(), item.getAmount());
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

    public void clear() {
        oracle.Support.emit("clear", owner.key, name);
    }

    public void refresh() {
        oracle.Support.emit("refresh", owner.key, name);
    }

    public void update() {
        oracle.Support.emit("update", owner.key, name);
    }
}
''')

adapter('core/cache/Cache.java', '''
package core.cache;

/** The cache: the item definition count is a recorded case answer. */
public final class Cache {
    public static int getItemDefinitionsSize() {
        return oracle.Support.askInt("itemDefinitionsSize", 0);
    }
}
''')

adapter('core/cache/def/impl/ItemDefinition.java', '''
package core.cache.def.impl;

import java.util.AbstractMap;
import java.util.Collections;
import java.util.Map;
import java.util.Set;

/** The definition table is definitions/: its size is a recorded case answer. */
public final class ItemDefinition {
    public static Map<Integer, ItemDefinition> getDefinitions() {
        int size = oracle.Support.askInt("itemDefs", 0);
        return new AbstractMap<Integer, ItemDefinition>() {
            @Override public Set<Map.Entry<Integer, ItemDefinition>> entrySet() { return Collections.emptySet(); }
            @Override public int size() { return size; }
        };
    }
}
''')

adapter('core/cache/def/impl/VarbitDefinition.java', '''
package core.cache.def.impl;

/** Varbit definitions (definitions/varbits): lookups are recorded case answers. */
public final class VarbitDefinition {
    private final int id;

    private VarbitDefinition(int id) {
        this.id = id;
    }

    static VarbitDefinition answer(Object id) {
        return id == null ? null : new VarbitDefinition(((Number) id).intValue());
    }

    public static VarbitDefinition forObjectID(int id) {
        return answer(oracle.Support.ask("varbitForObject", id, id));
    }

    public static VarbitDefinition forId(int id) {
        return answer(oracle.Support.ask("varbitForId", id, id));
    }

    public int getVarpId() {
        return oracle.Support.askInt("varbitVarp", 0, id);
    }

    public int getStartBit() {
        return oracle.Support.askInt("varbitStart", 0, id);
    }

    @Override
    public String toString() {
        return oracle.Support.askStr("varbitText", "varbit " + id, id);
    }
}
''')

adapter('core/cache/def/impl/SceneryDefinition.java', '''
package core.cache.def.impl;

/** Scenery definitions (definitions/scenery): lookups are recorded case answers. */
public final class SceneryDefinition {
    private final int id;

    private SceneryDefinition(int id) {
        this.id = id;
    }

    public static SceneryDefinition forId(int id) {
        oracle.Support.emit("sceneryDef", id);
        return new SceneryDefinition(id);
    }

    public int getVarbitID() {
        return oracle.Support.askInt("sceneryVarbit", -1, id);
    }

    public VarbitDefinition getConfigFile() {
        return VarbitDefinition.answer(oracle.Support.ask("sceneryConfigFile", null, id));
    }
}
''')

adapter('core/cache/def/impl/NPCDefinition.java', '''
package core.cache.def.impl;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** NPC definitions (definitions/npc): the table's values in the case's order, each [id, childIds or null]. */
public final class NPCDefinition {
    private final int id;
    private final int[] children;

    private NPCDefinition(int id, int[] children) {
        this.id = id;
        this.children = children;
    }

    public static Map<Integer, NPCDefinition> getDefinitions() {
        oracle.Support.emit("npcDefinitions");
        Map<Integer, NPCDefinition> out = new LinkedHashMap<>();
        int i = 0;
        for (Object o : oracle.Support.list("npcDefinitions")) {
            if (o == null) {
                out.put(-1 - i++, null);
                continue;
            }
            List row = (List) o;
            int[] children = null;
            if (row.get(1) != null) {
                List c = (List) row.get(1);
                children = new int[c.size()];
                for (int k = 0; k < children.length; k++) children[k] = ((Number) c.get(k)).intValue();
            }
            out.put(((Number) row.get(0)).intValue(), new NPCDefinition(((Number) row.get(0)).intValue(), children));
            i++;
        }
        return out;
    }

    public int getId() {
        return id;
    }

    public int[] getChildNPCIds() {
        return children;
    }
}
''')

adapter('core/game/node/scenery/Scenery.java', '''
package core.game.node.scenery;

import core.cache.def.impl.SceneryDefinition;
import core.game.node.Node;
import core.game.world.map.Location;

/** A scenery value: construction is a recorded call (the definition lookups and normalisation are Scenery's). */
public class Scenery extends Node {
    private final int id;
    private final Location location;
    private final int type;
    private final int rotation;
    private final SceneryDefinition definition;

    /* ORIGINAL_SCENERY_CONSTRUCTORS */

    public Scenery(int id, Location location, int type, int rotation) {
        oracle.Support.emit("newScenery", id, oracle.Support.coord(location), type, rotation);
        if (rotation < 0) {
            rotation += 4;
        }
        if (id < 1) {
            type = 22;
        }
        this.key = oracle.Support.sceneryKey();
        this.id = id;
        this.location = location;
        this.type = type;
        this.rotation = rotation;
        this.definition = SceneryDefinition.forId(id);
    }

    /** A scenery the case places in the world: [id, [x, y, z], type, rotation, constructed, renderable, active]. */
    public static Scenery placed(Object spec) {
        if (spec == null) return null;
        java.util.List row = (java.util.List) spec;
        boolean constructed = Boolean.TRUE.equals(row.get(4));
        Scenery s = constructed ? new Constructed(row) : new Scenery(row);
        return s;
    }

    Scenery(java.util.List row) {
        this.id = ((Number) row.get(0)).intValue();
        this.location = oracle.Support.location(row.get(1));
        this.type = ((Number) row.get(2)).intValue();
        this.rotation = ((Number) row.get(3)).intValue();
        this.definition = null;
        this.key = "placed" + id;
        this.renderable = Boolean.TRUE.equals(row.get(5));
        this.active = Boolean.TRUE.equals(row.get(6));
    }

    private boolean renderable = true;
    private boolean active = true;

    public int getId() {
        return id;
    }

    public Location getLocation() {
        return location;
    }

    public int getType() {
        return type;
    }

    public int getRotation() {
        return rotation;
    }

    public SceneryDefinition getDefinition() {
        return SceneryDefinition.forId(id);
    }

    public boolean isRenderable() {
        return renderable;
    }

    @Override
    public boolean isActive() {
        return active;
    }

    /* ORIGINAL_SCENERY_TO_STRING */
}
''')

adapter('core/game/node/scenery/Constructed.java', '''
package core.game.node.scenery;

/** A constructed scenery. */
public class Constructed extends Scenery {
    Constructed(java.util.List row) {
        super(row);
    }
}
''')

adapter('core/game/node/scenery/SceneryBuilder.java', '''
package core.game.node.scenery;

/** SceneryBuilder (world/scenery): add and remove are recorded calls. */
public final class SceneryBuilder {
    public static Constructed add(Scenery object) {
        oracle.Support.emit("sceneryAdd", object.key);
        return null;
    }

    public static boolean remove(Scenery object) {
        return oracle.Support.askBool("sceneryRemove", true, object.key, object.getId());
    }
}
''')

adapter('core/game/world/map/Direction.java', '''
package core.game.world.map;

/** The original's constants, in order. */
public enum Direction {
    /* ORIGINAL_DIRECTION_NAMES */
}
''')

adapter('core/game/world/map/Location.kt', '''
package core.game.world.map

/** A location value; the members below the constructor are the original's text. */
class Location(val x: Int, val y: Int, val z: Int) {
    override fun equals(other: Any?): Boolean = other is Location && other.x == x && other.y == y && other.z == z
    override fun hashCode(): Int = (z shl 30) or (x shl 15) or y

    /** Location.getRegion: RegionManager.forId(regionId). */
    val region: Region
        get() = RegionManager.forId(regionId)

    /* ORIGINAL_LOCATION_MEMBERS */

    companion object {
        @JvmStatic
        fun create(x: Int, y: Int, z: Int): Location = Location(x, y, z)
    }
}
''')

adapter('core/game/world/map/Region.java', '''
package core.game.world.map;

/** A region: its activity is a recorded case answer. */
public class Region {
    private final int id;

    public Region(int id) {
        this.id = id;
    }

    public boolean isActive() {
        return oracle.Support.askBool("regionActive", false, id);
    }
}
''')

adapter('core/game/world/map/build/DynamicRegion.java', '''
package core.game.world.map.build;

/** A dynamic region. */
public class DynamicRegion extends core.game.world.map.Region {
    public DynamicRegion(int id) {
        super(id);
    }
}
''')

adapter('core/game/world/map/RegionChunk.java', '''
package core.game.world.map;

import core.game.node.scenery.Scenery;
import java.util.List;

/** A chunk's object arrays from the case. */
public class RegionChunk {
    private final String at;

    RegionChunk(String at) {
        this.at = at;
    }

    public Scenery[][][] getStatObjects() {
        Scenery[][][] out = new Scenery[8][8][4];
        List rows = oracle.Support.list("statObjects:" + at);
        for (Object o : rows) {
            List r = (List) o;
            out[((Number) r.get(0)).intValue()][((Number) r.get(1)).intValue()][((Number) r.get(2)).intValue()] = Scenery.placed(r.get(3));
        }
        return out;
    }

    public Scenery[] getObjects(int chunkOffsetX, int chunkOffsetY) {
        oracle.Support.emit("chunkObjects", at, chunkOffsetX, chunkOffsetY);
        List rows = oracle.Support.list("chunkObjects:" + at);
        Scenery[] out = new Scenery[rows.size()];
        for (int i = 0; i < out.length; i++) out[i] = Scenery.placed(rows.get(i));
        return out;
    }
}
''')

adapter('core/game/world/map/RegionManager.kt', '''
package core.game.world.map

import core.game.node.scenery.Scenery
import core.game.world.map.build.DynamicRegion

/** RegionManager's queries are world/region_queries and scene: recorded case answers. */
object RegionManager {
    @JvmStatic
    fun forId(regionId: Int): Region {
        oracle.Support.emit("region", regionId)
        return if (oracle.Support.answer("dynamic", false, regionId) == true) DynamicRegion(regionId) else Region(regionId)
    }

    @JvmStatic
    fun getObject(l: Location): Scenery? = Scenery.placed(oracle.Support.ask("object", null, l.x, l.y, l.z))

    @JvmStatic
    fun getRegionChunk(l: Location): RegionChunk {
        oracle.Support.emit("chunk", l.x, l.y, l.z)
        return RegionChunk("" + (l.x shr 3) + "," + (l.y shr 3) + "," + l.z)
    }

    @JvmStatic
    fun isTeleportPermitted(loc: Location): Boolean = oracle.Support.askBool("teleportPermitted", true, loc.x, loc.y, loc.z)

    @JvmStatic
    fun getClippingFlag(loc: Location, projectile: Boolean = false): Int =
        oracle.Support.askInt("clippingFlag", 0, loc.x, loc.y, loc.z, projectile)

    @JvmStatic
    fun isLandscape(loc: Location): Boolean = oracle.Support.askBool("landscape", false, loc.x, loc.y, loc.z)
}
''')

adapter('core/worker/MajorUpdateWorker.kt', '''
package core.worker

/** The tick worker (world/driver): started is a case value; start, stop and setStarted are recorded. */
class MajorUpdateWorker {
    private var flag = oracle.Support.caseValue("workerStarted") != false

    var started: Boolean
        get() = flag
        set(value) {
            oracle.Support.emit("workerSetStarted", value)
            flag = value
        }

    fun start() {
        oracle.Support.emit("workerStart")
        flag = true
    }

    fun stop() {
        oracle.Support.emit("workerStop")
    }
}
''')

adapter('core/game/world/GameWorld.kt', '''
package core.game.world

import core.ServerStore
import core.api.PersistWorld
import core.api.ShutdownListener
import core.auth.UserAccountInfo
import core.game.node.entity.player.Player
import core.game.system.task.Pulse
import core.worker.MajorUpdateWorker

/** The world's settings, pulse runner, worker, listeners, stores, account storage and authenticator. */
object GameWorld {
    class Settings(val isDevMode: Boolean, val isBeta: Boolean)

    class Runner {
        fun submit(pulse: Pulse) {
            oracle.Support.submit(pulse)
        }
    }

    class Storage {
        fun checkUsernameTaken(username: String): Boolean = oracle.Support.askBool("taken", false, username)

        fun getAccountInfo(username: String): UserAccountInfo {
            oracle.Support.emit("accountInfo", username)
            return oracle.Support.account(username)
        }

        fun getUsernamesWithIP(ip: String): List<String> {
            oracle.Support.emit("ips", ip)
            return oracle.Support.list("ips:" + ip).map { it as String }
        }

        fun update(info: UserAccountInfo) {
            oracle.Support.emit("accountUpdate", oracle.Support.accountKey(info))
        }
    }

    class Authenticator {
        fun checkPassword(player: Player, password: String): Boolean = oracle.Support.askBool("checkPassword", false, player.key, password)

        fun updatePassword(username: String, newPassword: String) {
            oracle.Support.emit("updatePassword", username, newPassword)
        }
    }

    class Listener(private val label: String) : ShutdownListener {
        override fun shutdown() {
            oracle.Support.emit("shutdownListener", label)
        }
    }

    class Persist(private val label: String) : PersistWorld {
        override fun parse() {
            oracle.Support.emit("persistParse", label)
        }

        override fun save() {
            oracle.Support.emit("persistSave", label)
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

    @JvmStatic
    val majorUpdateWorker = MajorUpdateWorker()

    @JvmStatic
    val shutdownListeners = ArrayList<ShutdownListener>(oracle.Support.list("shutdownListeners").map { Listener(it as String) })

    @JvmStatic
    val worldPersists = ArrayList<PersistWorld>(oracle.Support.list("worldPersists").map {
        if (it == "ServerStore") ServerStore() else Persist(it as String)
    })
}
''')

adapter('core/game/world/repository/DisconnectionQueue.kt', '''
package core.game.world.repository

/** The disconnection queue (world/disconnection): every call is a recorded case answer. */
class DisconnectionQueue {
    fun isEmpty(): Boolean = oracle.Support.askBool("queueEmpty", true)

    fun update() {
        oracle.Support.emit("queueUpdate")
    }

    fun clear() {
        oracle.Support.emit("queueClear")
    }
}
''')

adapter('core/game/world/repository/Repository.kt', '''
package core.game.world.repository

import core.game.node.entity.player.Player

/* ORIGINAL_GE_NEWS_MUTE */

/** The repository (world/repository): the name map, the player list and news are recorded calls. */
object Repository {
    @JvmStatic
    val players: List<Player>
        get() = oracle.Support.list("online").mapNotNull { oracle.Support.player(it) }

    @JvmStatic
    val disconnectionQueue = DisconnectionQueue()

    @JvmStatic
    fun getPlayerByName(name: String?): Player? = oracle.Support.player(oracle.Support.ask("byName", null, name))

    @JvmStatic
    fun sendNews(string: String, icon: Int = 12, color: String = "CC6600") {
        oracle.Support.emit("news", string, icon, color)
    }

    /* ORIGINAL_SEND_NEWS_1 */
}
''')

adapter('core/api/ContentAPI.kt', '''
package core.api

import core.game.component.Component
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.repository.Repository
import core.tools.Log

/** The ContentAPI members the selected code calls: the attribute and interface helpers are the original's
 *  one-line delegations; every other member is a recorded call (handlers, tasks and timers are kept). */
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
    return entity.getAttribute(attribute, default)
}

fun <T> setAttribute(entity: Entity, attribute: String, value: T) {
    entity.setAttribute(attribute, value)
}

fun removeAttribute(entity: Entity, attribute: String) {
    entity.removeAttribute(attribute.replace("/save:",""))
}

fun openInterface(player: Player, id: Int) {
    player.interfaceManager.open(Component(id))
}

fun closeInterface(player: Player) {
    player.interfaceManager.close()
}

fun sendNews(message: String) {
    Repository.sendNews(message, 12, "CC6600")
}

/** A definition-table read: answered, not recorded. */
fun getItemName(id: Int): String = oracle.Support.answer("itemDefName", "item " + id, id) as String

fun addItemOrDrop(player: Player, id: Int, amount: Int = 1) {
    oracle.Support.emit("addItemOrDrop", player.key, id, amount)
}

@JvmOverloads
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("setVarp", player.key, varpIndex, value, save)
}

@JvmOverloads
fun setVarbit(player: Player, varbitId: Int, value: Int, save: Boolean = false) {
    oracle.Support.emit("setVarbit", player.key, varbitId, value, save)
}

fun getVarbit(player: Player, varbitId: Int): Int = oracle.Support.askInt("getVarbit", 0, player.key, varbitId)

inline fun <reified T> getOrStartTimer(entity: Entity, vararg args: Any): T {
    oracle.Support.emit("timer", entity.key, T::class.java.simpleName)
    return T::class.java.getConstructor(String::class.java).newInstance(entity.key)
}

fun log(origin: Class<*>, type: Log, message: String) {
    oracle.Support.emit("log", origin.name, type.name, message)
}

/** A slayer task: its enum name. */
class OracleTask(val name: String)

fun getSlayerTask(player: Player): OracleTask? {
    val name = oracle.Support.ask("slayerTask", null, player.key) as String?
    return if (name == null) null else OracleTask(name)
}
''')

adapter('content/data/ChargedItem.kt', '''
package content.data

/** ChargedItem is content data: forId, forCharge and getCharge are recorded case answers. */
class ChargedItem(private val id: Int) {
    fun forCharge(charge: Int): Int = oracle.Support.askInt("forCharge", id, id, charge)

    companion object {
        @JvmStatic
        fun forId(id: Int): ChargedItem? = if (oracle.Support.askBool("chargedItem", false, id)) ChargedItem(id) else null

        @JvmStatic
        fun getCharge(id: Int): Int? = (oracle.Support.ask("chargedCharge", null, id) as Number?)?.toInt()
    }
}
''')

adapter('core/tools/SystemLogger.kt', '''
package core.tools

/** Imported by the selected sources; logging goes through core.api.log. */
object SystemLogger

/* ORIGINAL_LOG_ENUM */
''')

adapter('core/game/system/security/EncryptionManager.java', '''
package core.game.system.security;

/** SystemManager's static field: never used by the selected code. */
public final class EncryptionManager {
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

adapter('core/net/IoSession.java', '''
package core.net;

/** The player's session (transport, native): what a writer writes is recorded with its bytes. */
public final class IoSession {
    private final String owner;

    public IoSession(String owner) {
        this.owner = owner;
    }

    public IsaacKeyPair getIsaacPair() {
        return new IsaacKeyPair();
    }

    public void write(Object message) {
        oracle.Support.emit("write", owner, oracle.Packets.written(message));
    }
}
''')

adapter('core/net/IsaacKeyPair.java', '''
package core.net;

import core.cache.crypto.ISAACCipher;

/** The session's cipher pair: the output stream adds nothing to the opcode here. */
public final class IsaacKeyPair {
    public ISAACCipher getOutput() {
        return new ISAACCipher();
    }
}
''')

adapter('core/cache/crypto/ISAACCipher.java', '''
package core.cache.crypto;

/** The transport's ISAAC stream (native in the session): opcodes are recorded before it. */
public final class ISAACCipher {
    public int getNextValue() {
        return 0;
    }
}
''')

adapter('core/game/world/update/flag/context/Animation.java', '''
package core.game.world.update.flag.context;

import core.game.node.scenery.Scenery;

/** An object animation: its id and object. */
public final class Animation {
    private final int id;
    private final Scenery object;

    public Animation(int id, Scenery object) {
        this.id = id;
        this.object = object;
    }

    public int getId() {
        return id;
    }

    public Scenery getObject() {
        return object;
    }
}
''')