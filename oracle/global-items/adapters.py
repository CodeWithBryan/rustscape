"""Recording adapters for the global-items oracle.

Each entry is a source file compiled with the unchanged originals listed in generate.py. `/* ORIGINAL_* */` markers are
replaced by original text read from the pinned sources at generation time. Everything else records the call in
oracle.Support and answers from the case. Field names differ from the getters' Kotlin properties so that Kotlin reads go
through the getters, as in the original.
"""

ADAPTERS = {
    # ------------------------------------------------------------------ nodes
    'core/game/node/Node.java': r'''package core.game.node;
import core.game.world.map.Location;
public class Node {
    public String key, nodeName;
    public int nodeId;
    public Location nodeLocation;
    public int getId() { return nodeId; }
    public String getName() { return nodeName; }
    public Location getLocation() { return nodeLocation; }
    public boolean isActive() { return true; }
    public core.game.node.entity.npc.NPC asNpc() { return (core.game.node.entity.npc.NPC) this; }
    public core.game.node.item.Item asItem() { return (core.game.node.item.Item) this; }
    @Override public String toString() { return key; }
}
''',
    'core/game/node/entity/Entity.java': r'''package core.game.node.entity;
import core.game.node.Node;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import oracle.Support;
import java.util.Map;
public class Entity extends Node {
    public final Map<String, Object> attrs = new java.util.TreeMap<>();
    public core.game.interaction.ScriptProcessor scripts = new core.game.interaction.ScriptProcessor();
    final core.game.node.entity.impl.Animator animatorObject = new core.game.node.entity.impl.Animator();
    final core.game.node.entity.impl.Properties propertiesObject = new core.game.node.entity.impl.Properties();
    final core.game.node.entity.player.link.TeleportManager teleporterObject = new core.game.node.entity.player.link.TeleportManager();
    final core.game.node.entity.combat.ImpactHandler impactObject = new core.game.node.entity.combat.ImpactHandler();
    final core.game.node.entity.impl.PulseManager pulseObject = new core.game.node.entity.impl.PulseManager();
    final core.game.world.map.zone.ZoneMonitor zoneObject = new core.game.world.map.zone.ZoneMonitor();
    public core.game.node.entity.impl.Animator getAnimator() { return animatorObject; }
    public core.game.node.entity.impl.Properties getProperties() { return propertiesObject; }
    public core.game.node.entity.player.link.TeleportManager getTeleporter() { return teleporterObject; }
    public core.game.node.entity.combat.ImpactHandler getImpactHandler() { return impactObject; }
    public core.game.node.entity.impl.PulseManager getPulseManager() { return pulseObject; }
    public core.game.world.map.zone.ZoneMonitor getZoneMonitor() { return zoneObject; }
    public boolean face(Entity entity) { Support.emit("face", Support.key(entity)); return true; }
    public void faceLocation(Location l) { Support.emit("faceLocation", Support.coord(l)); }
    public void faceTemporary(Entity entity, int duration) { Support.emit("faceTemporary", Support.key(entity), duration); }
    public void lock() { Support.emit("lockAll"); }
    public void lock(int time) { Support.emit("lock", time); }
    public void unlock() { Support.emit("unlock"); }
    public boolean animate(Animation animation) { return animatorObject.animate(animation); }
    public boolean graphics(Graphics graphics) { return animatorObject.graphics(graphics); }
    public boolean visualize(Animation animation, Graphics graphics) { return animatorObject.animate(animation, graphics); }
    public boolean isTeleBlocked() { Support.emit("isTeleBlocked"); return Support.answerBool("isTeleBlocked", false); }
    public void dispatch(core.game.event.Event event) {
        core.game.event.TeleportEvent t = (core.game.event.TeleportEvent) event;
        Support.emit("teleportEvent", t.getType().name(), t.getMethod().name(), Support.key(t.getSource()), Support.coord(t.getLocation()));
    }
    // GameAttributes: "/save:" is stripped on set and read.
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String k, T fail) {
        Support.emit("getAttribute", k);
        Object object = attrs.get(k.replace("/save:", ""));
        return object != null ? (T) object : fail;
    }
    @SuppressWarnings("unchecked")
    public <T> T getAttribute(String k) { Support.emit("getAttribute", k); return (T) attrs.get(k.replace("/save:", "")); }
    public void setAttribute(String k, Object v) {
        Support.emit("setAttribute", k, Support.value(v));
        attrs.put(k.startsWith("/save:") ? k.substring(6) : k, v);
    }
}
''',
    'core/game/node/entity/player/Player.java': r'''package core.game.node.entity.player;
import oracle.Support;
public class Player extends core.game.node.entity.Entity {
    final core.game.node.entity.player.link.Settings settingsObject = new core.game.node.entity.player.link.Settings();
    final core.game.node.entity.player.link.PacketDispatch dispatchObject = new core.game.node.entity.player.link.PacketDispatch();
    final core.game.node.entity.player.link.diary.AchievementDiaryManager diaryObject = new core.game.node.entity.player.link.diary.AchievementDiaryManager();
    final content.global.skill.construction.HouseManager houseObject = new content.global.skill.construction.HouseManager();
    final core.game.container.Container inventoryObject = new core.game.container.Container();
    final core.game.dialogue.DialogueInterpreter interpreterObject = new core.game.dialogue.DialogueInterpreter();
    public core.game.node.entity.player.link.Settings getSettings() { return settingsObject; }
    public core.game.node.entity.player.link.PacketDispatch getPacketDispatch() { return dispatchObject; }
    public core.game.node.entity.player.link.diary.AchievementDiaryManager getAchievementDiaryManager() { return diaryObject; }
    public content.global.skill.construction.HouseManager getHouseManager() { return houseObject; }
    public core.game.container.Container getInventory() { return inventoryObject; }
    public core.game.dialogue.DialogueInterpreter getDialogueInterpreter() { return interpreterObject; }
    public String getUsername() { Support.emit("username"); return Support.text("username", "Tracer"); }
    public void sendMessage(String text) { Support.emit("message", text); }
}
''',
    'core/game/node/entity/npc/NPC.java': r'''package core.game.node.entity.npc;
public class NPC extends core.game.node.entity.Entity {
    public NPC() {}
    public NPC(int id) { oracle.Support.emit("newNpc", id); nodeId = id; }
}
''',
    'core/game/node/scenery/Scenery.java': 'package core.game.node.scenery;\npublic class Scenery extends core.game.node.Node {}\n',
    'core/game/node/item/Item.java': r'''package core.game.node.item;
public class Item extends core.game.node.Node {
    private int amount = 1;
    public int itemSlot = -1;
    public Item() {}
    public Item(int id) { this(id, 1, 1000); }
    public Item(int id, int amount) { this(id, amount, 1000); }
    public Item(int id, int amount, int charge) { nodeId = id; this.amount = amount; oracle.Support.newItem(this, id, amount); }
    public void oracleAmount(int value) { amount = value; }
    public int getAmount() { return amount; }
    public int getSlot() { return itemSlot; }
    /* ORIGINAL_ITEM_TO_STRING */
}
''',
    'core/cache/def/impl/ItemDefinition.java': r'''package core.cache.def.impl;
import java.util.Map;
public class ItemDefinition {
    final int definitionId;
    ItemDefinition(int id) { definitionId = id; }
    public static ItemDefinition forId(int id) { return new ItemDefinition(id); }
    public Map<String, Object> getHandlers() {
        return new java.util.TreeMap<String, Object>() {
            @Override public Object put(String k, Object v) { oracle.Support.itemHandler(definitionId, k, v); return null; }
        };
    }
}
''',
    'core/cache/def/impl/SceneryDefinition.java': r'''package core.cache.def.impl;
public class SceneryDefinition {
    public static SceneryDefinition forId(int id) { return new SceneryDefinition(); }
    public int[] getChildrenIds() { return null; }
}
''',
    'core/game/world/map/Location.java': r'''package core.game.world.map;
public class Location extends core.game.node.Node {
    final int lx, ly, lz;
    public Location(int x, int y) { this(x, y, 0); }
    public Location(int x, int y, int z) { lx = x; ly = y; lz = z; }
    public static Location create(int x, int y, int z) { return new Location(x, y, z); }
    public static Location create(Location l) { return new Location(l.getX(), l.getY(), l.getZ()); }
    public int getX() { return lx; }
    public int getY() { return ly; }
    public int getZ() { return lz; }
}
''',
    # ------------------------------------------------------------------ entity members
    'core/game/world/update/flag/context/Animation.java': r'''package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator.Priority;
public class Animation {
    public static final Animation RESET = new Animation(-1, Priority.VERY_HIGH);
    final int animationId, animationDelay;
    final Priority animationPriority;
    public Animation(int id) { this(id, 0, Priority.MID); }
    public Animation(int id, Priority priority) { this(id, 0, priority); }
    public Animation(int id, int delay) { this(id, delay, Priority.MID); }
    public Animation(int id, int delay, Priority priority) { animationId = id; animationDelay = delay; animationPriority = priority; }
    public int getId() { return animationId; }
    public int getDuration() { oracle.Support.emit("animationDuration", animationId); return oracle.Support.answerInt("duration:" + animationId, 1); }
    public java.util.List<Object> oracle() { return java.util.Arrays.asList(animationId, animationDelay, animationPriority == null ? null : animationPriority.name()); }
}
''',
    'core/game/world/update/flag/context/Graphics.java': r'''package core.game.world.update.flag.context;
public class Graphics {
    final int graphicsId, graphicsHeight, graphicsDelay;
    public Graphics(int id) { this(id, 0, 0); }
    public Graphics(int id, int height) { this(id, height, 0); }
    public Graphics(int id, int height, int delay) { graphicsId = id; graphicsHeight = height; graphicsDelay = delay; }
    public java.util.List<Object> oracle() { return java.util.Arrays.asList(graphicsId, graphicsHeight, graphicsDelay); }
}
''',
    'core/game/node/entity/impl/Animator.java': r'''package core.game.node.entity.impl;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import oracle.Support;
public class Animator {
    public static enum Priority { LOW, MID, HIGH, VERY_HIGH }
    public boolean animate(Animation a) { Support.emit("animate", a.oracle()); return true; }
    public boolean animate(Animation a, Graphics g) { Support.emit("visualize", a.oracle(), g.oracle()); return true; }
    public boolean forceAnimation(Animation a) { Support.emit("forceAnimation", a.oracle()); return true; }
    public boolean graphics(Graphics g) { Support.emit("graphics", g.oracle()); return true; }
}
''',
    'core/game/node/entity/impl/PulseType.java': 'package core.game.node.entity.impl;\npublic enum PulseType { STANDARD, STRONG }\n',
    'core/game/node/entity/impl/PulseManager.java': r'''package core.game.node.entity.impl;
public class PulseManager {
    public void clear(PulseType type) { oracle.Support.emit("pulseClear", type == null ? null : type.name()); }
}
''',
    'core/game/node/entity/impl/Properties.java': r'''package core.game.node.entity.impl;
import core.game.world.map.Location;
public class Properties {
    Location teleportTarget;
    public Location getTeleportLocation() { return teleportTarget; }
    public void setTeleportLocation(Location l) { oracle.Support.emit("setTeleportLocation", oracle.Support.coord(l)); teleportTarget = l; }
}
''',
    'core/game/node/entity/combat/ImpactHandler.java': r'''package core.game.node.entity.combat;
public class ImpactHandler {
    int disabled;
    public int getDisabledTicks() { return disabled; }
    public void setDisabledTicks(int ticks) { oracle.Support.emit("disabledTicks", ticks); disabled = ticks; }
}
''',
    'core/game/interaction/ScriptProcessor.java': r'''package core.game.interaction;
public class ScriptProcessor {
    int scriptDelay;
    public int getDelay() { return scriptDelay; }
    public void setDelay(int delay) { oracle.Support.emit("scriptDelay", delay); scriptDelay = delay; }
}
''',
    'core/game/node/entity/player/link/TeleportManager.java': r'''package core.game.node.entity.player.link;
import core.game.world.map.Location;
public class TeleportManager {
    public static enum TeleportType { NORMAL, ANCIENT, LUNAR, TELETABS, OBELISK, TELE_OTHER, FAIRY_RING, PURO_PURO, ECTOPHIAL, CHRISTMAS, CABBAGE, ENTRANA_MAGIC_DOOR, MINIGAME, INSTANT, RC_GUILD }
    public boolean send(Location location, TeleportType type) {
        oracle.Support.emit("teleportSend", oracle.Support.coord(location), type == null ? null : type.name());
        return oracle.Support.answerBool("teleportSend", true);
    }
}
''',
    'core/game/world/map/zone/ZoneMonitor.java': r'''package core.game.world.map.zone;
public class ZoneMonitor {
    public boolean teleport(int type, core.game.node.Node node) { oracle.Support.emit("zoneTeleport", type, oracle.Support.key(node)); return oracle.Support.answerBool("zoneTeleport", true); }
}
''',
    'core/game/world/map/zone/impl/WildernessZone.java': r'''package core.game.world.map.zone.impl;
public class WildernessZone {
    public static boolean checkTeleport(core.game.node.entity.player.Player p, int level) { oracle.Support.emit("checkTeleport", level); return oracle.Support.answerBool("checkTeleport", true); }
}
''',
    'core/game/node/entity/player/link/PacketDispatch.java': r'''package core.game.node.entity.player.link;
public class PacketDispatch {
    public void sendMessage(String text) { oracle.Support.emit("message", text); }
    public void sendMessages(String... texts) { oracle.Support.emit("messages", oracle.Support.values(texts)); }
}
''',
    'core/game/node/entity/player/link/Settings.java': r'''package core.game.node.entity.player.link;
public class Settings {
    public double getRunEnergy() { oracle.Support.emit("runEnergy"); return ((Number) oracle.Support.answer("runEnergy", 50.0)).doubleValue(); }
    public void updateRunEnergy(double delta) { oracle.Support.emit("updateRunEnergy", delta); }
}
''',
    'core/game/node/entity/player/link/diary/DiaryType.java': 'package core.game.node.entity.player.link.diary;\npublic enum DiaryType { VARROCK }\n',
    'core/game/node/entity/player/link/diary/AchievementDiaryManager.java': r'''package core.game.node.entity.player.link.diary;
public class AchievementDiaryManager {
    public void finishTask(core.game.node.entity.player.Player player, DiaryType type, int level, int task) { oracle.Support.emit("finishTask", type.name(), level, task); }
}
''',
    'core/game/container/Container.java': r'''package core.game.container;
import core.game.node.item.Item;
import oracle.Support;
public class Container {
    public boolean remove(Item item) { Support.emit("inventoryRemove", Support.key(item)); return Support.answerBool("inventoryRemove", true); }
    public boolean add(Item item) { Support.emit("inventoryAdd", Support.key(item)); return Support.answerBool("inventoryAdd", true); }
    public Item get(int slot) { Support.emit("inventoryGet", slot); return (Item) Support.node(Support.answer("inventoryGet", null)); }
}
''',
    'content/global/skill/construction/HouseLocation.java': r'''package content.global.skill.construction;
public class HouseLocation {
    public core.game.world.map.Location getExitLocation() { oracle.Support.emit("houseExitLocation"); return oracle.Support.location(oracle.Support.answer("houseExitLocation", null)); }
}
''',
    'content/global/skill/construction/HouseManager.java': r'''package content.global.skill.construction;
import core.game.node.entity.player.Player;
import oracle.Support;
public class HouseManager {
    final HouseLocation where = new HouseLocation();
    public HouseLocation getLocation() { return where; }
    public void preEnter(Player player, boolean buildingMode) { Support.emit("housePreEnter", buildingMode); }
    public core.game.world.map.Location getEnterLocation() {
        Support.emit("houseEnterLocation");
        return Support.location(Support.answer("houseEnterLocation", java.util.Arrays.asList(1950L, 5730L, 0L)));
    }
    public void postEnter(Player player, boolean buildingMode) { Support.emit("housePostEnter", buildingMode); }
}
''',
    # ------------------------------------------------------------------ dialogue
    'core/game/component/Component.java': 'package core.game.component;\npublic class Component {}\n',
    'core/game/dialogue/FacialExpression.java': 'package core.game.dialogue;\npublic enum FacialExpression { NEUTRAL, HALF_GUILTY, FRIENDLY }\n',
    'core/game/dialogue/DialogueInterpreter.java': r'''package core.game.dialogue;
import oracle.Support;
public class DialogueInterpreter {
    public static void add(int id, DialoguePlugin plugin) { Support.dialogueAdd(id, plugin); }
    public boolean open(int key, Object... args) { Support.emit("openDialoguePlugin", key, Support.values(args)); return Support.answerBool("openDialoguePlugin", true); }
    public core.game.component.Component sendOptions(Object title, String... options) { Support.emit("sendOptions", title, Support.values(options)); return null; }
    public boolean close() { Support.emit("interpreterClose"); return true; }
}
''',
    'core/game/dialogue/DialoguePlugin.java': r'''package core.game.dialogue;
import core.game.node.entity.player.Player;
public abstract class DialoguePlugin implements core.plugin.Plugin<Player> {
    protected Player player;
    protected DialogueInterpreter interpreter;
    protected int stage;
    public DialoguePlugin() {}
    /* ORIGINAL_DIALOGUE_PLUGIN_CONSTRUCTOR */
    public abstract DialoguePlugin newInstance(Player player);
    public abstract boolean open(Object... args);
    public abstract boolean handle(int interfaceId, int buttonId);
    public abstract int[] getIds();
    /* ORIGINAL_DIALOGUE_PLUGIN_INIT */
    /* ORIGINAL_DIALOGUE_PLUGIN_END */
    public Object fireEvent(String identifier, Object... args) { return null; }
    public int oracleStage() { return stage; }
}
''',
    'core/game/dialogue/DialogueFile.kt': r'''package core.game.dialogue

import core.game.node.entity.npc.NPC
import core.game.node.entity.player.Player
import core.tools.START_DIALOGUE

abstract class DialogueFile {
    var player: Player? = null
    var npc: NPC? = null
    var interpreter: DialogueInterpreter? = null
    open var stage = START_DIALOGUE

    abstract fun handle(componentID: Int, buttonID: Int)

    /* ORIGINAL_DIALOGUE_FILE_END */
}
''',
    # ------------------------------------------------------------------ registries
    'core/game/interaction/InteractionListener.kt': r'''package core.game.interaction

import core.api.ContentInterface
import core.game.node.Node
import core.game.node.entity.player.Player

interface InteractionListener : ContentInterface {
    fun defineListeners()
    fun on(id: Int, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(intArrayOf(id), type.name, arrayOf(*option), handler)
    }
    fun on(ids: IntArray, type: IntType, vararg option: String, handler: (player: Player, node: Node) -> Boolean) {
        oracle.Support.on(ids, type.name, arrayOf(*option), handler)
    }
    fun onUseWith(type: IntType, used: Int, vararg with: Int, handler: (player: Player, used: Node, with: Node) -> Boolean) {
        oracle.Support.onUseWith(type.name, used, with, handler)
    }
}
''',
    'core/api/ContentInterface.java': 'package core.api;\npublic interface ContentInterface {}\n',
    'core/plugin/Initializable.java': 'package core.plugin;\npublic @interface Initializable {}\n',
    'core/tools/Log.kt': 'package core.tools\n\nenum class Log { FINE, INFO, WARN, ERR }\n',
    'core/tools/SystemLogger.kt': 'package core.tools\n\nobject SystemLogger\n',
    'core/game/node/entity/player/info/LogType.kt': 'package core.game.node.entity.player.info\n\nenum class LogType { DUPE_ALERT }\n',
    'core/game/node/entity/player/info/PlayerMonitor.kt': r'''package core.game.node.entity.player.info

import core.game.node.entity.player.Player

object PlayerMonitor {
    @JvmStatic fun log(player: Player, type: LogType, details: String) { oracle.Support.emit("playerMonitorLog", type.name, details) }
}
''',
    'core/game/event/Event.kt': 'package core.game.event\n\ninterface Event\n',
    'core/game/event/TeleportEvent.kt': r'''package core.game.event

import content.global.skill.magic.TeleportMethod
import core.game.node.entity.player.link.TeleportManager.TeleportType
import core.game.world.map.Location

/* ORIGINAL_TELEPORT_EVENT */
''',
    # ------------------------------------------------------------------ world
    'core/game/system/task/PulseRunner.java': r'''package core.game.system.task;
public class PulseRunner {
    public void submit(Pulse pulse) { oracle.Support.emit("submitWorld", oracle.Support.track(pulse)); }
}
''',
    'core/game/world/GameWorld.kt': r'''package core.game.world

import core.game.system.task.PulseRunner

object GameWorld {
    @JvmStatic
    val ticks: Int get() = oracle.Support.intValue("ticks", 0)
    @JvmStatic
    var Pulser = PulseRunner()
}
''',
    'core/ServerConstants.kt': r'''package core

import core.game.world.map.Location

class ServerConstants {
    companion object {
        @JvmField
        var HOME_LOCATION: Location? = Location.create(3222, 3218, 0)
    }
}
''',
    'core/ServerStore.kt': r'''package core

import org.json.simple.JSONObject

class ServerStore {
    companion object {
        fun getArchive(name: String): JSONObject = oracle.Support.archive(name)

        /* ORIGINAL_STORE_GET_INT */
    }
}
''',
    'core/game/node/entity/skill/Skills.java': r'''package core.game.node.entity.skill;
public class Skills {
    /* ORIGINAL_SKILL_IDS */
}
''',
    'content/data/Quests.java': 'package content.data;\npublic enum Quests { HAUNTED_MINE, GHOSTS_AHOY, MOURNINGS_END_PART_I }\n',
    # ------------------------------------------------------------------ content services
    'content/global/skill/slayer/SlayerManager.kt': r'''package content.global.skill.slayer

import core.game.node.entity.player.Player

class SlayerMaster(val npc: Int)
class SlayerFlags {
    val taskFlags: Int get() { oracle.Support.emit("slayerTaskFlags"); return oracle.Support.answerInt("slayerTaskFlags", 0) }
}
class SlayerManager {
    companion object {
        fun getInstance(player: Player): SlayerManager = SlayerManager()
    }
    fun hasTask(): Boolean { oracle.Support.emit("slayerHasTask"); return oracle.Support.answerBool("slayerHasTask", true) }
    val master: SlayerMaster? get() {
        oracle.Support.emit("slayerMaster")
        val npc = oracle.Support.answer("slayerMaster", 8273) ?: return null
        return SlayerMaster((npc as Number).toInt())
    }
    val flags: SlayerFlags get() = SlayerFlags()
}
''',
    'content/global/skill/slayer/SlayerUtils.kt': r'''package content.global.skill.slayer

object SlayerUtils {
    @JvmStatic
    /* ORIGINAL_PLURALISE */
}
''',
    'content/global/skill/runecrafting/Altar.kt': r'''package content.global.skill.runecrafting

enum class Altar { AIR, MIND, WATER, EARTH, FIRE, BODY, COSMIC, LAW, NATURE, CHAOS, ASTRAL, DEATH, BLOOD, OURANIA }
''',
    'content/global/skill/runecrafting/MysteriousRuinListener.kt': r'''package content.global.skill.runecrafting

import core.game.node.entity.player.Player

class MysteriousRuinListener {
    companion object {
        fun checkReq(player: Player, altar: Altar): Boolean { oracle.Support.emit("altarRequirement", altar.name); return oracle.Support.answerBool("altarRequirement", true) }
    }
}
''',
    'content/region/kandarin/ardougne/quest/plaguecity/PlagueCityListeners.kt': r'''package content.region.kandarin.ardougne.quest.plaguecity

class PlagueCityListeners {
    companion object {
        /* ORIGINAL_ARDOUGNE_TELE_ATTRIBUTE */
    }
}
''',
    'content/global/skill/magic/modern/ModernListeners.kt': r'''package content.global.skill.magic.modern

import core.game.node.entity.player.Player
import core.game.node.item.Item

class ModernListeners {
    fun alchemize(player: Player, item: Item, high: Boolean, explorersRing: Boolean = false): Boolean {
        oracle.Support.emit("alchemize", oracle.Support.key(item), high, explorersRing)
        return oracle.Support.answerBool("alchemize", true)
    }
}
''',
    'content/data/consumables/effects/PrayerEffect.kt': r'''package content.data.consumables.effects

import core.game.node.entity.player.Player

class PrayerEffect @JvmOverloads constructor(var base: Double, var bonus: Double, private val roundUp: Boolean = false) {
    fun activate(player: Player?) { oracle.Support.emit("prayerEffect", base, bonus, roundUp) }
}
''',
    # ------------------------------------------------------------------ ContentAPI
    'core/api/ContentAPI.kt': r'''package core.api

import core.game.dialogue.DialogueAction
import core.game.dialogue.FacialExpression
import core.game.interaction.QueueStrength
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.impl.Animator
import core.game.node.entity.player.Player
import core.game.node.entity.player.link.TeleportManager
import core.game.node.item.Item
import core.game.world.GameWorld
import core.game.world.map.Location
import core.game.world.update.flag.context.Animation
import core.game.world.update.flag.context.Graphics
import core.tools.Log
import content.data.Quests
import oracle.Support

fun sendMessage(player: Player, message: String) { Support.emit("message", message) }
fun sendDialogue(player: Player, message: String) { Support.emit("sendDialogue", message) }
fun openDialogue(player: Player, dialogue: Any, vararg args: Any) { Support.openDialogue(player, dialogue, arrayOf(*args)) }
@JvmOverloads
fun hasRequirement(player: Player, quest: Quests, message: Boolean = true): Boolean {
    Support.emit("hasRequirement", quest.name, message)
    return Support.answerBool("hasRequirement:" + quest.name, true)
}
fun isQuestComplete(player: Player, quest: Quests): Boolean {
    Support.emit("isQuestComplete", quest.name)
    return Support.answerBool("isQuestComplete:" + quest.name, true)
}
fun hasLevelStat(player: Player, skill: Int, level: Int): Boolean { Support.emit("hasLevelStat", skill, level); return Support.answerBool("hasLevelStat", true) }
fun queueScript(entity: Entity, delay: Int = 1, strength: QueueStrength = QueueStrength.WEAK, persist: Boolean = false, script: (stage: Int) -> Boolean) {
    Support.emit("queueScript", delay, strength.name, persist, Support.track(script))
}
fun delayScript(entity: Entity, ticks: Int): Boolean { Support.emit("delayScript", ticks); return Support.answerBool("delayScript", false) }
fun stopExecuting(entity: Entity): Boolean { Support.emit("stopExecuting"); return Support.answerBool("stopExecuting", true) }
fun playAudio(player: Player, id: Int, delay: Int = 0, loops: Int = 1, location: Location? = null, radius: Int = 8) {
    Support.emit("audio", id, delay, loops, Support.coord(location), radius)
}
@JvmOverloads
fun playGlobalAudio(location: Location, id: Int, delay: Int = 0, loops: Int = 1, radius: Int = 8) {
    Support.emit("globalAudio", Support.coord(location), id, delay, loops, radius)
}
fun closeInterface(player: Player) { Support.emit("closeInterface") }
fun closeAllInterfaces(player: Player) { Support.emit("closeAllInterfaces") }
fun <T> removeItem(player: Player, item: T, container: Container = Container.INVENTORY): Boolean {
    Support.emit("removeItem", if (item is Item) Support.key(item) else item, container.name)
    return Support.answerBool("removeItem", true)
}
fun replaceSlot(player: Player, slot: Int, item: Item, currentItem: Item? = null, container: Container = Container.INVENTORY): Item? {
    Support.emit("replaceSlot", slot, Support.key(item), Support.key(currentItem), container.name)
    return Support.node(Support.answer("replaceSlot", Support.key(currentItem))) as Item?
}
fun addItemOrDrop(player: Player, id: Int, amount: Int = 1) { Support.emit("addItemOrDrop", id, amount) }
fun addItem(player: Player, id: Int, amount: Int = 1, container: Container = Container.INVENTORY): Boolean {
    Support.emit("addItem", id, amount, container.name)
    return Support.answerBool("addItem", true)
}
fun inInventory(player: Player, id: Int, amount: Int = 1): Boolean { Support.emit("inInventory", id, amount); return Support.answerBool("inInventory", true) }
fun log(origin: Class<*>, type: Log, message: String) { Support.emit("log", origin.name, type.name, message) }
fun sendNPCDialogue(player: Player, npc: Int, msg: String, expr: FacialExpression = FacialExpression.NEUTRAL, hide: Boolean = false) {
    Support.emit("npcDialogue", npc, msg, expr.name, hide)
}
fun setVarp(player: Player, varpIndex: Int, value: Int, save: Boolean = false) { Support.emit("setVarp", varpIndex, value, save) }
fun getSlayerTaskName(player: Player): String { Support.emit("slayerTaskName"); return Support.answer("slayerTaskName", "goblin") as String }
fun getSlayerTaskKillsRemaining(player: Player): Int { Support.emit("slayerKillsRemaining"); return Support.answerInt("slayerKillsRemaining", 12) }
fun addDialogueAction(player: Player, action: DialogueAction) { Support.emit("addDialogueAction", Support.track(action)) }
fun sendItemSelect(player: Player, vararg options: String, keepAlive: Boolean = false, callback: (slot: Int, optionIndex: Int) -> Unit) {
    Support.emit("sendItemSelect", Support.values(arrayOf<Any>(*options)), keepAlive, Support.track(callback))
}

/* ORIGINAL_CONTENT_API */
''',
}
